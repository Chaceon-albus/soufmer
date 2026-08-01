use std::{
    sync::{Arc, Mutex},
    thread,
};

use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::{
    domain::{
        AppError, BackendEvent, ErrorCode, EventName, InitializationProgress, InitializationStep,
        ProgressValue, TaskAcknowledgement,
    },
    jobs::{
        BatchRunnerEvent, BatchRuntime, FfprobeInputProber, ProductionBatchExecutor,
        SequentialBatchRunner, preflight_plan,
    },
    process::CancellationToken,
    runtime::AppPaths,
};

pub struct TaskManager {
    active: Mutex<Option<Arc<ActiveTask>>>,
}

pub fn start_batch(
    app: AppHandle,
    manager: Arc<TaskManager>,
    request: crate::domain::StartBatchRequest,
) -> Result<TaskAcknowledgement, AppError> {
    let paths = AppPaths::discover()?;
    let runtime = BatchRuntime::resolve(&paths)?;
    let task = manager.begin()?;
    let acknowledgement = TaskAcknowledgement {
        task_id: task.id.clone(),
        accepted_at: chrono_timestamp(),
    };
    thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let prober = FfprobeInputProber {
                ffprobe: runtime.ffprobe.clone(),
                logs: runtime.logs.clone(),
                cancellation: task.cancelled.clone(),
            };
            let plan = preflight_plan(&request, &task.id, &prober)?;
            let runner = SequentialBatchRunner {
                app_paths: paths,
                runtime,
                executor: Arc::new(ProductionBatchExecutor),
            };
            let result = runner.run(&task.id, plan, &task.cancelled, |event| match event {
                BatchRunnerEvent::Progress(payload) => {
                    emit(&app, &task, EventName::BatchProgress, payload)
                }
                BatchRunnerEvent::ItemCompleted(payload) => {
                    emit(&app, &task, EventName::BatchItemCompleted, payload)
                }
                BatchRunnerEvent::Completed(payload) => {
                    emit(&app, &task, EventName::BatchCompleted, payload)
                }
                BatchRunnerEvent::Failed(error) => emit_terminal_failure(&app, &task, error),
                BatchRunnerEvent::Cancelled => {}
            });
            if result.cancelled {
                emit(
                    &app,
                    &task,
                    EventName::TaskCancelled,
                    AppError::new(
                        ErrorCode::TaskCancelled,
                        "batch cancelled after publishing completed results",
                    ),
                );
            }
            Ok::<(), AppError>(())
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => emit_terminal_failure(&app, &task, error),
            Err(_) => emit_terminal_failure(
                &app,
                &task,
                AppError::new(ErrorCode::InferenceFailed, "batch worker thread panicked"),
            ),
        }
        manager.finish(&task.id);
    });
    Ok(acknowledgement)
}
impl Default for TaskManager {
    fn default() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }
}

struct ActiveTask {
    id: String,
    cancelled: CancellationToken,
    sequence: Mutex<u64>,
}
impl ActiveTask {
    fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            cancelled: CancellationToken::new(),
            sequence: Mutex::new(0),
        }
    }
    fn next_sequence(&self) -> u64 {
        let mut sequence = self.sequence.lock().expect("task sequence lock poisoned");
        *sequence += 1;
        *sequence
    }
}

impl TaskManager {
    fn begin(&self) -> Result<Arc<ActiveTask>, AppError> {
        let mut active = self.active.lock().expect("task manager lock poisoned");
        if active.is_some() {
            return Err(AppError::new(
                ErrorCode::TaskAlreadyActive,
                "another task is already active",
            ));
        }
        let task = Arc::new(ActiveTask::new());
        *active = Some(Arc::clone(&task));
        Ok(task)
    }
    pub fn cancel(&self, task_id: &str) -> Result<(), AppError> {
        let active = self.active.lock().expect("task manager lock poisoned");
        let Some(task) = active.as_ref() else {
            return Err(AppError::new(
                ErrorCode::TaskCancelled,
                "task is already terminal",
            ));
        };
        if task.id != task_id {
            return Err(AppError::new(
                ErrorCode::InvalidRequest,
                "task ID does not match the active task",
            ));
        }
        task.cancelled.cancel();
        Ok(())
    }
    fn finish(&self, task_id: &str) {
        let mut active = self.active.lock().expect("task manager lock poisoned");
        if active.as_ref().is_some_and(|task| task.id == task_id) {
            *active = None;
        }
    }
}

pub fn start_initialization(
    app: AppHandle,
    manager: Arc<TaskManager>,
) -> Result<TaskAcknowledgement, AppError> {
    let task = manager.begin()?;
    let acknowledgement = TaskAcknowledgement {
        task_id: task.id.clone(),
        accepted_at: chrono_timestamp(),
    };
    thread::spawn(move || {
        let result = crate::runtime::AppPaths::discover().and_then(|paths| {
            crate::runtime::initialize(&paths, &task.cancelled, &mut |update| {
                let (bytes_completed, bytes_total, bytes_per_second) = update
                    .bytes
                    .map_or((None, None, None), |(completed, total, speed)| {
                        (Some(completed), total, Some(speed))
                    });
                emit(
                    &app,
                    &task,
                    EventName::RuntimeProgress,
                    InitializationProgress {
                        runtime_version: "private-runtime".into(),
                        step_index: step_index(update.step),
                        step_count: 7,
                        step_id: update.step,
                        overall: ProgressValue::Determinate {
                            fraction: update.fraction,
                        },
                        current: update.current,
                        bytes_completed,
                        bytes_total,
                        bytes_per_second,
                        detail: Some(update.detail.into()),
                    },
                );
            })
        });
        match result {
            Ok(()) => match crate::runtime::AppPaths::discover()
                .and_then(|paths| crate::runtime::environment_status(&paths))
            {
                Ok(status @ crate::domain::EnvironmentStatus::Ready { .. }) => {
                    emit(&app, &task, EventName::RuntimeCompleted, status)
                }
                Ok(_) => emit_terminal_failure(
                    &app,
                    &task,
                    AppError::new(
                        ErrorCode::EnvironmentNotInitialized,
                        "initializer finished without a ready runtime",
                    ),
                ),
                Err(error) => emit_terminal_failure(&app, &task, error),
            },
            Err(error) if error.code == ErrorCode::TaskCancelled => {
                emit(&app, &task, EventName::TaskCancelled, error)
            }
            Err(error) => emit_terminal_failure(&app, &task, error),
        }
        manager.finish(&task.id);
    });
    Ok(acknowledgement)
}
fn step_index(step: InitializationStep) -> u8 {
    match step {
        InitializationStep::CheckingSystem => 1,
        InitializationStep::PreparingTools => 2,
        InitializationStep::InstallingPython => 3,
        InitializationStep::SyncingEnvironment => 4,
        InitializationStep::DownloadingModel => 5,
        InitializationStep::SelfTesting => 6,
        InitializationStep::Activating => 7,
    }
}

fn emit<T: serde::Serialize + Clone>(
    app: &AppHandle,
    task: &ActiveTask,
    name: EventName,
    payload: T,
) {
    let event = BackendEvent {
        schema_version: crate::domain::events::EVENT_SCHEMA_VERSION,
        task_id: task.id.clone(),
        sequence: task.next_sequence(),
        timestamp: chrono_timestamp(),
        event_type: name.as_str().into(),
        payload,
    };
    if let Err(error) = app.emit(name.as_str(), event) {
        tracing::warn!(%error, "could not emit backend event");
    }
}

fn emit_terminal_failure(app: &AppHandle, task: &ActiveTask, error: AppError) {
    match AppPaths::discover().and_then(|paths| crate::diagnostics::persist(&paths, &error)) {
        Ok(()) => {}
        Err(persist_error) => tracing::warn!(
            diagnostic_id = %error.diagnostic_id,
            detail = %persist_error.technical_detail,
            "could not persist terminal task diagnostic"
        ),
    }
    emit(app, task, EventName::TaskFailed, error);
}
fn chrono_timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

#[cfg(test)]
mod tests {
    use super::TaskManager;
    #[test]
    fn cancellation_marks_only_the_active_task() {
        let manager = TaskManager::default();
        let task = manager.begin().unwrap();
        manager.cancel(&task.id).unwrap();
        assert!(task.cancelled.is_cancelled());
        manager.finish(&task.id);
        assert!(manager.cancel(&task.id).is_err());
    }
}
