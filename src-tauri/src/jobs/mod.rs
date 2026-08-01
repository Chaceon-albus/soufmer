use std::{
    fs,
    os::windows::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use serde::Deserialize;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

use crate::{
    audio::{
        AudioSourceInfo, JobPaths, cleanup_job, compatibility_residual_args, create_job_paths,
        is_supported_input, model_input_args, parse_source_info, source_native_args,
        source_rate_residual_args,
    },
    domain::{
        AppError, BatchItemResult, BatchProgress, BatchResult, BatchStage, ConflictPolicy,
        ErrorCode, InputMode, OutputFormat, ProcessingMode, ProgressValue, StartBatchRequest,
    },
    process::{
        CancellationToken, ProcessRunner, ProcessSpec, external_process_path,
        external_process_path_string,
    },
    progress::duration_weighted_fraction,
    runtime::AppPaths,
};

#[derive(Clone, Debug)]
pub struct EnumeratedInput {
    pub absolute_path: PathBuf,
    pub relative_path: PathBuf,
}

pub fn enumerate_inputs(request: &StartBatchRequest) -> Result<Vec<EnumeratedInput>, AppError> {
    let input = PathBuf::from(&request.input_path);
    if !input.exists() {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "input path does not exist",
        ));
    }
    let output = PathBuf::from(&request.output_directory);
    let output_in_input = input.is_dir()
        && output.exists()
        && is_within_case_insensitive(&canonical(&output)?, &canonical(&input)?);
    let mut inputs = Vec::new();
    match request.input_mode {
        InputMode::File => {
            if !input.is_file() || !is_supported_input(&input) {
                return Err(AppError::new(
                    ErrorCode::InputUnsupported,
                    "input is not a supported audio file",
                ));
            }
            inputs.push(EnumeratedInput {
                absolute_path: canonical(&input)?,
                relative_path: input.file_name().map(PathBuf::from).unwrap_or_default(),
            });
        }
        InputMode::Folder => collect_folder(
            &input,
            &input,
            &output,
            output_in_input,
            request.recursive,
            &mut inputs,
        )?,
    }
    inputs.sort_by(|left, right| {
        normalized_sort_key(&left.relative_path).cmp(&normalized_sort_key(&right.relative_path))
    });
    Ok(inputs)
}

fn collect_folder(
    root: &Path,
    current: &Path,
    output: &Path,
    output_in_input: bool,
    recursive: bool,
    inputs: &mut Vec<EnumeratedInput>,
) -> Result<(), AppError> {
    for entry in fs::read_dir(current).map_err(job_error)? {
        let entry = entry.map_err(job_error)?;
        let file_type = entry.file_type().map_err(job_error)?;
        let path = entry.path();
        if file_type.is_symlink()
            || entry.metadata().map_err(job_error)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT
                != 0
        {
            continue;
        }
        if file_type.is_dir() {
            if recursive
                && !(output_in_input
                    && is_within_case_insensitive(&canonical(&path)?, &canonical(output)?))
            {
                collect_folder(root, &path, output, output_in_input, true, inputs)?;
            }
            continue;
        }
        if file_type.is_file() && is_supported_input(&path) && !is_generated_output(&path) {
            let absolute_path = canonical(&path)?;
            let relative_path = absolute_path
                .strip_prefix(canonical(root)?)
                .map_err(job_error)?
                .to_path_buf();
            inputs.push(EnumeratedInput {
                absolute_path,
                relative_path,
            });
        }
    }
    Ok(())
}

fn canonical(path: &Path) -> Result<PathBuf, AppError> {
    fs::canonicalize(path).map_err(job_error)
}
fn is_within_case_insensitive(path: &Path, ancestor: &Path) -> bool {
    let path = path
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    let ancestor = ancestor
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase();
    path == ancestor || path.starts_with(&(ancestor + "\\"))
}
fn normalized_sort_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}
fn is_generated_output(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().contains(" (instrumental"))
}

#[derive(Clone, Debug)]
pub struct PlannedOutput {
    pub mode: ProcessingMode,
    pub final_path: PathBuf,
    pub partial_path: PathBuf,
    pub skip: bool,
    pub overwrite: bool,
    pub format: OutputFormat,
}
#[derive(Clone, Debug)]
pub struct PlannedItem {
    pub item_index: u32,
    pub input: EnumeratedInput,
    pub source_info: AudioSourceInfo,
    pub outputs: Vec<PlannedOutput>,
}
#[derive(Clone, Debug)]
pub struct BatchPlan {
    pub items: Vec<PlannedItem>,
    pub preflight_failures: Vec<BatchItemResult>,
    pub output_directory: PathBuf,
    pub total_duration_seconds: f64,
    pub skipped: usize,
    pub total_input_count: usize,
}

pub trait InputProber: Send + Sync {
    fn probe(&self, input: &Path) -> Result<AudioSourceInfo, AppError>;
}
#[derive(Clone)]
pub struct FfprobeInputProber {
    pub ffprobe: PathBuf,
    pub logs: PathBuf,
    pub cancellation: CancellationToken,
}
impl InputProber for FfprobeInputProber {
    fn probe(&self, input: &Path) -> Result<AudioSourceInfo, AppError> {
        let mut spec = ProcessSpec::new(&self.ffprobe);
        spec.arguments = vec![
            "-v".into(),
            "error".into(),
            "-show_streams".into(),
            "-show_format".into(),
            "-of".into(),
            "json".into(),
            external_process_path(input)?.into_os_string(),
        ];
        spec.stderr_log = Some(
            self.logs
                .join(format!("ffprobe-{}.stderr.log", uuid::Uuid::new_v4())),
        );
        let output = ProcessRunner::run(spec, self.cancellation.clone(), Arc::new(|_| {}))?;
        if output.exit_code != Some(0) {
            return Err(AppError::new(
                ErrorCode::InputUnsupported,
                "FFprobe inspection failed",
            ));
        }
        parse_source_info(&output.stdout_lines.join("\n"))
    }
}

pub fn preflight_plan(
    request: &StartBatchRequest,
    task_id: &str,
    prober: &dyn InputProber,
) -> Result<BatchPlan, AppError> {
    ensure_output_directory(Path::new(&request.output_directory))?;
    let output_root = canonical(Path::new(&request.output_directory))?;
    let inputs = enumerate_inputs(request)?;
    let total_input_count = inputs.len();
    let mut items = Vec::new();
    let mut failures = Vec::new();
    let mut total_duration_seconds = 0.0;
    let mut skipped = 0;
    for (index, input) in inputs.into_iter().enumerate() {
        let source_info = match prober.probe(&input.absolute_path) {
            Ok(info) => info,
            Err(error) => {
                if error.code == ErrorCode::TaskCancelled {
                    return Err(error);
                }
                failures.push(BatchItemResult {
                    item_index: (index + 1) as u32,
                    input_path: readable_path(&input.absolute_path),
                    outputs: Vec::new(),
                    duration_seconds: 0.0,
                    warnings: Vec::new(),
                    error_code: Some(error.code),
                });
                continue;
            }
        };
        let item_output_directory = if request.preserve_directory_structure {
            input
                .relative_path
                .parent()
                .map(|relative| output_root.join(relative))
                .unwrap_or_else(|| output_root.clone())
        } else {
            output_root.clone()
        };
        ensure_output_directory(&item_output_directory)?;
        let outputs = plan_item_outputs(
            request,
            task_id,
            &input.absolute_path,
            &item_output_directory,
        )?;
        if outputs.iter().all(|output| output.skip) {
            skipped += 1;
            continue;
        }
        total_duration_seconds += source_info.duration_seconds;
        items.push(PlannedItem {
            item_index: (index + 1) as u32,
            input,
            source_info,
            outputs,
        });
    }
    if items.is_empty() && skipped == 0 && failures.is_empty() {
        return Err(AppError::new(
            ErrorCode::InputUnsupported,
            "no audio files were found",
        ));
    }
    Ok(BatchPlan {
        items,
        preflight_failures: failures,
        output_directory: output_root,
        total_duration_seconds,
        skipped,
        total_input_count,
    })
}

fn ensure_output_directory(directory: &Path) -> Result<(), AppError> {
    fs::create_dir_all(directory).map_err(|_| {
        AppError::new(
            ErrorCode::OutputNotWritable,
            "could not create output directory",
        )
    })?;
    let probe = directory.join(format!(".soufmer-write-check-{}", uuid::Uuid::new_v4()));
    fs::write(&probe, []).map_err(|_| {
        AppError::new(
            ErrorCode::OutputNotWritable,
            "output directory is not writable",
        )
    })?;
    fs::remove_file(probe).map_err(|_| {
        AppError::new(
            ErrorCode::OutputNotWritable,
            "could not remove output directory write check",
        )
    })
}

fn plan_item_outputs(
    request: &StartBatchRequest,
    task_id: &str,
    input: &Path,
    output_directory: &Path,
) -> Result<Vec<PlannedOutput>, AppError> {
    let modes = if request.generate_both_modes {
        vec![
            ProcessingMode::Compatibility44100,
            ProcessingMode::SourceSampleRate,
        ]
    } else {
        vec![request.processing_mode]
    };
    modes
        .into_iter()
        .map(|mode| {
            plan_one_output(
                output_directory,
                input,
                task_id,
                request.output_format,
                request.conflict_policy,
                mode,
                request.generate_both_modes,
            )
        })
        .collect()
}
fn plan_one_output(
    directory: &Path,
    input: &Path,
    task_id: &str,
    format: OutputFormat,
    conflict: ConflictPolicy,
    mode: ProcessingMode,
    both: bool,
) -> Result<PlannedOutput, AppError> {
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::new(ErrorCode::InvalidRequest, "source file has no valid stem"))?;
    let extension = match format {
        OutputFormat::Flac => "flac",
        OutputFormat::WavFloat32 => "wav",
    };
    let suffix = if both {
        match mode {
            ProcessingMode::Compatibility44100 => " (Instrumental - 44.1k)",
            ProcessingMode::SourceSampleRate => " (Instrumental - Source SR)",
        }
    } else {
        " (Instrumental)"
    };
    let base = format!("{stem}{suffix}");
    let mut final_path = directory.join(format!("{base}.{extension}"));
    let skip = conflict == ConflictPolicy::Skip && final_path.exists();
    if conflict == ConflictPolicy::AutoNumber {
        let mut number = 2;
        while final_path.exists() {
            final_path = directory.join(format!("{base} ({number}).{extension}"));
            number += 1;
        }
    }
    if final_path == input {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "planned output would overwrite the input",
        ));
    }
    Ok(PlannedOutput {
        mode,
        partial_path: directory.join(format!(".{stem}.{task_id}.{:?}.partial.{extension}", mode)),
        final_path,
        skip,
        overwrite: conflict == ConflictPolicy::Overwrite,
        format,
    })
}

#[derive(Clone, Debug)]
pub struct BatchRuntime {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub python: PathBuf,
    pub worker_module: String,
    pub worker_cwd: PathBuf,
    pub checkpoint: PathBuf,
    pub config: PathBuf,
    pub environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    pub logs: PathBuf,
}
impl BatchRuntime {
    pub fn resolve(paths: &AppPaths) -> Result<Self, AppError> {
        let active = crate::runtime::resolve_active_runtime(paths)?;
        Ok(Self {
            ffmpeg: active.ffmpeg,
            ffprobe: active.ffprobe,
            python: active.python,
            worker_module: active.worker_module,
            worker_cwd: active.worker_cwd,
            checkpoint: active.checkpoint,
            config: active.config,
            environment: active.environment,
            logs: active.logs,
        })
    }
    pub fn validate(&self) -> Result<(), AppError> {
        if !self.ffmpeg.is_file()
            || !self.ffprobe.is_file()
            || !self.python.is_file()
            || !self.checkpoint.is_file()
            || !self.config.is_file()
            || !self.worker_cwd.is_dir()
            || self.worker_module.trim().is_empty()
        {
            return Err(AppError::new(
                ErrorCode::EnvironmentNotInitialized,
                "validated private runtime paths are required",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerJsonlEnvelope {
    pub schema_version: u8,
    #[serde(rename = "type")]
    pub event_type: String,
    pub task_id: String,
    pub payload: WorkerJsonlPayload,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerJsonlPayload {
    pub stage: Option<String>,
    pub current: Option<u64>,
    pub total: Option<u64>,
}
pub fn parse_worker_progress(line: &str) -> Result<WorkerJsonlPayload, AppError> {
    let event: WorkerJsonlEnvelope = serde_json::from_str(line).map_err(|_| {
        AppError::new(
            ErrorCode::InferenceFailed,
            "worker emitted invalid JSON Lines progress",
        )
    })?;
    if event.schema_version != 1 || event.event_type != "progress" || event.task_id.is_empty() {
        return Err(AppError::new(
            ErrorCode::InferenceFailed,
            "worker emitted an unsupported progress event",
        ));
    }
    Ok(event.payload)
}

pub trait BatchExecutor: Send + Sync {
    fn prepare_model_input(
        &self,
        runtime: &BatchRuntime,
        item: &PlannedItem,
        paths: &JobPaths,
        cancellation: &CancellationToken,
    ) -> Result<(), AppError>;
    fn run_worker_direct(
        &self,
        task_id: &str,
        runtime: &BatchRuntime,
        item: &PlannedItem,
        paths: &JobPaths,
        cancellation: &CancellationToken,
        on_jsonl: &mut dyn FnMut(&str),
    ) -> Result<(), AppError>;
    fn build_output(
        &self,
        runtime: &BatchRuntime,
        item: &PlannedItem,
        paths: &JobPaths,
        output: &PlannedOutput,
        cancellation: &CancellationToken,
    ) -> Result<(), AppError>;
    fn validate_and_publish(
        &self,
        runtime: &BatchRuntime,
        item: &PlannedItem,
        output: &PlannedOutput,
        cancellation: &CancellationToken,
    ) -> Result<(), AppError>;
}
pub struct ProductionBatchExecutor;
impl ProductionBatchExecutor {
    fn ffmpeg(
        runtime: &BatchRuntime,
        args: Vec<std::ffi::OsString>,
        cwd: &Path,
        log: PathBuf,
        cancellation: &CancellationToken,
    ) -> Result<(), AppError> {
        let mut spec = ProcessSpec::new(&runtime.ffmpeg);
        spec.arguments = args;
        spec.current_dir = Some(cwd.to_path_buf());
        spec.environment = runtime.environment.clone();
        spec.stderr_log = Some(log);
        let result = ProcessRunner::run(spec, cancellation.clone(), Arc::new(|_| {}))
            .map_err(|error| map_process_error(error, ErrorCode::PostprocessFailed))?;
        if result.exit_code == Some(0) {
            Ok(())
        } else {
            Err(AppError::new(
                ErrorCode::PostprocessFailed,
                "FFmpeg process failed",
            ))
        }
    }
}
impl BatchExecutor for ProductionBatchExecutor {
    fn prepare_model_input(
        &self,
        runtime: &BatchRuntime,
        item: &PlannedItem,
        paths: &JobPaths,
        cancellation: &CancellationToken,
    ) -> Result<(), AppError> {
        Self::ffmpeg(
            runtime,
            model_input_args(
                &external_process_path(&item.input.absolute_path)?,
                &item.source_info,
                &external_process_path(&paths.model_input)?,
            ),
            &paths.root,
            paths.logs.join("prepare.stderr.log"),
            cancellation,
        )?;
        let info = FfprobeInputProber {
            ffprobe: runtime.ffprobe.clone(),
            logs: paths.logs.clone(),
            cancellation: cancellation.clone(),
        }
        .probe(&paths.model_input)
        .map_err(|error| map_process_error(error, ErrorCode::PostprocessFailed))?;
        if info.sample_rate != 44_100
            || info.channels != 2
            || info.sample_format.as_deref() != Some("flt")
        {
            return Err(AppError::new(
                ErrorCode::PostprocessFailed,
                "model input properties are invalid",
            ));
        }
        Ok(())
    }
    fn run_worker_direct(
        &self,
        task_id: &str,
        runtime: &BatchRuntime,
        _: &PlannedItem,
        paths: &JobPaths,
        cancellation: &CancellationToken,
        on_jsonl: &mut dyn FnMut(&str),
    ) -> Result<(), AppError> {
        uuid::Uuid::parse_str(task_id).map_err(|_| {
            AppError::new(ErrorCode::InferenceFailed, "batch task ID is not a UUID")
        })?;
        let request = paths.root.join("request.json");
        let body = worker_request_json(
            task_id,
            &paths.model_input,
            &paths.vocals,
            &runtime.checkpoint,
            &runtime.config,
        )?;
        let temporary = paths
            .root
            .join(format!(".request-{}.tmp", uuid::Uuid::new_v4()));
        fs::write(
            &temporary,
            serde_json::to_vec(&body).map_err(|_| {
                AppError::new(
                    ErrorCode::InferenceFailed,
                    "worker request serialization failed",
                )
            })?,
        )
        .map_err(job_error)?;
        fs::rename(&temporary, &request).map_err(job_error)?;
        let lines = Arc::new(std::sync::Mutex::new(Vec::new()));
        let capture = Arc::clone(&lines);
        let spec = worker_process_spec(runtime, paths, &request)?;
        let result = ProcessRunner::run(
            spec,
            cancellation.clone(),
            Arc::new(move |line| {
                capture
                    .lock()
                    .expect("worker line lock poisoned")
                    .push(line);
            }),
        )?;
        let lines = lines.lock().expect("worker line lock poisoned").clone();
        validate_worker_lifecycle(&lines, task_id, &paths.vocals, on_jsonl)?;
        let vocals = FfprobeInputProber {
            ffprobe: runtime.ffprobe.clone(),
            logs: paths.logs.clone(),
            cancellation: cancellation.clone(),
        }
        .probe(&paths.vocals)
        .map_err(|error| AppError::new(ErrorCode::InferenceFailed, error.technical_detail))?;
        if vocals.sample_rate != 44_100
            || vocals.channels != 2
            || vocals.sample_format.as_deref() != Some("flt")
            || vocals.codec_name != "pcm_f32le"
        {
            return Err(AppError::new(
                ErrorCode::InferenceFailed,
                "worker vocals output is not 44.1 kHz stereo Float32 WAV",
            ));
        }
        if result.exit_code == Some(0) {
            Ok(())
        } else {
            Err(AppError::new(
                ErrorCode::InferenceFailed,
                "worker exited unsuccessfully",
            ))
        }
    }
    fn build_output(
        &self,
        runtime: &BatchRuntime,
        item: &PlannedItem,
        paths: &JobPaths,
        output: &PlannedOutput,
        cancellation: &CancellationToken,
    ) -> Result<(), AppError> {
        let args = match output.mode {
            ProcessingMode::Compatibility44100 => compatibility_residual_args(
                &external_process_path(&paths.model_input)?,
                &external_process_path(&paths.vocals)?,
                &external_process_path(&output.partial_path)?,
                output.format,
                &item.source_info,
            ),
            ProcessingMode::SourceSampleRate => {
                Self::ffmpeg(
                    runtime,
                    source_native_args(
                        &external_process_path(&item.input.absolute_path)?,
                        &item.source_info,
                        &external_process_path(&paths.source_native)?,
                    ),
                    &paths.root,
                    paths.logs.join("source-native.stderr.log"),
                    cancellation,
                )?;
                source_rate_residual_args(
                    &external_process_path(&paths.source_native)?,
                    &external_process_path(&paths.vocals)?,
                    &external_process_path(&output.partial_path)?,
                    output.format,
                    &item.source_info,
                )
            }
        };
        Self::ffmpeg(
            runtime,
            args,
            &paths.root,
            paths.logs.join("residual.stderr.log"),
            cancellation,
        )
    }
    fn validate_and_publish(
        &self,
        runtime: &BatchRuntime,
        item: &PlannedItem,
        output: &PlannedOutput,
        cancellation: &CancellationToken,
    ) -> Result<(), AppError> {
        if cancellation.is_cancelled() {
            let _ = fs::remove_file(&output.partial_path);
            return Err(AppError::new(
                ErrorCode::TaskCancelled,
                "output publication cancelled",
            ));
        }
        let info = FfprobeInputProber {
            ffprobe: runtime.ffprobe.clone(),
            logs: runtime.logs.clone(),
            cancellation: cancellation.clone(),
        }
        .probe(&output.partial_path)
        .map_err(|error| map_process_error(error, ErrorCode::PostprocessFailed))?;
        tracing::debug!(
            source_sample_rate = item.source_info.sample_rate,
            output_sample_rate = info.sample_rate,
            source_duration_seconds = item.source_info.duration_seconds,
            output_duration_seconds = info.duration_seconds,
            duration_delta_seconds = info.duration_seconds - item.source_info.duration_seconds,
            channels = info.channels,
            codec = %info.codec_name,
            mode = ?output.mode,
            "validated generated output against source properties"
        );
        let expected_rate = if matches!(output.mode, ProcessingMode::Compatibility44100) {
            44_100
        } else {
            item.source_info.sample_rate
        };
        if info.sample_rate != expected_rate
            || info.channels != 2
            || info.duration_seconds <= 0.0
            || (info.duration_seconds - item.source_info.duration_seconds).abs() > 0.25
            || (matches!(output.format, OutputFormat::WavFloat32)
                && info.sample_format.as_deref() != Some("flt"))
            || (matches!(output.format, OutputFormat::Flac)
                && (info.codec_name != "flac"
                    || !matches!(
                        info.bits_per_raw_sample.or(info.bits_per_sample),
                        Some(16 | 24)
                    )))
        {
            return Err(AppError::new(
                ErrorCode::PostprocessFailed,
                "partial output properties are invalid",
            ));
        }
        publish_partial(&output.partial_path, &output.final_path, output.overwrite)
    }
}

fn worker_process_spec(
    runtime: &BatchRuntime,
    paths: &JobPaths,
    request: &Path,
) -> Result<ProcessSpec, AppError> {
    let request = external_process_path(request)?;
    let temp = paths.root.join("tmp");
    fs::create_dir_all(&temp).map_err(job_error)?;
    let mut environment = runtime.environment.clone();
    for name in ["TEMP", "TMP"] {
        environment.retain(|(existing, _)| !existing.to_string_lossy().eq_ignore_ascii_case(name));
        environment.push((name.into(), temp.clone().into_os_string()));
    }
    let mut spec = ProcessSpec::new(&runtime.python);
    spec.arguments = vec![
        "-I".into(),
        "-m".into(),
        runtime.worker_module.clone().into(),
        "separate".into(),
        "--request".into(),
        request.into_os_string(),
    ];
    spec.current_dir = Some(runtime.worker_cwd.clone());
    spec.environment = environment;
    spec.stderr_log = Some(paths.logs.join("worker.stderr.log"));
    Ok(spec)
}
fn absolute(path: &Path) -> Result<String, AppError> {
    if path.exists() {
        return fs::canonicalize(path)
            .map_err(job_error)
            .and_then(|value| external_process_path_string(&value));
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new(ErrorCode::InferenceFailed, "path has no parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| AppError::new(ErrorCode::InferenceFailed, "path has no filename"))?;
    fs::canonicalize(parent)
        .map_err(job_error)
        .and_then(|parent| external_process_path_string(&parent.join(name)))
}

fn map_process_error(error: AppError, fallback: ErrorCode) -> AppError {
    if matches!(
        error.code,
        ErrorCode::TaskCancelled | ErrorCode::PathUnsupported
    ) {
        error
    } else {
        AppError::new(fallback, error.technical_detail)
    }
}
fn readable_path(path: &Path) -> String {
    external_process_path_string(path).unwrap_or_else(|_| path.display().to_string())
}
fn worker_request_json(
    task_id: &str,
    input: &Path,
    vocals: &Path,
    checkpoint: &Path,
    config: &Path,
) -> Result<serde_json::Value, AppError> {
    Ok(
        serde_json::json!({"schemaVersion":1,"taskId":task_id,"inputPath":absolute(input)?,"outputVocalsPath":absolute(vocals)?,"checkpointPath":absolute(checkpoint)?,"configPath":absolute(config)?,"device":"cuda:0","batchSize":1,"overlap":4}),
    )
}
fn validate_worker_lifecycle(
    lines: &[String],
    task_id: &str,
    vocals: &Path,
    on_jsonl: &mut dyn FnMut(&str),
) -> Result<(), AppError> {
    let mut state = 0_u8;
    for line in lines {
        let event: serde_json::Value = serde_json::from_str(line).map_err(|_| {
            AppError::new(ErrorCode::InferenceFailed, "worker emitted malformed JSONL")
        })?;
        if event
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            != Some(1)
            || event.get("taskId").and_then(serde_json::Value::as_str) != Some(task_id)
        {
            return Err(AppError::new(
                ErrorCode::InferenceFailed,
                "worker event schema or task mismatch",
            ));
        }
        let kind = event
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                AppError::new(ErrorCode::InferenceFailed, "worker event type missing")
            })?;
        match kind {
            "ready"
                if state == 0
                    && event
                        .pointer("/payload/device")
                        .and_then(serde_json::Value::as_str)
                        == Some("cuda:0") =>
            {
                state = 1
            }
            "stage"
                if state == 1
                    && event
                        .pointer("/payload/stage")
                        .and_then(serde_json::Value::as_str)
                        == Some("loadingModel") =>
            {
                state = 2
            }
            "stage"
                if state == 2
                    && event
                        .pointer("/payload/stage")
                        .and_then(serde_json::Value::as_str)
                        == Some("separating") =>
            {
                state = 3
            }
            "progress" if state == 3 => {
                let current = event
                    .pointer("/payload/current")
                    .and_then(serde_json::Value::as_u64);
                let total = event
                    .pointer("/payload/total")
                    .and_then(serde_json::Value::as_u64);
                if total.is_none_or(|total| total == 0)
                    || current
                        .zip(total)
                        .is_none_or(|(current, total)| current > total)
                {
                    return Err(AppError::new(
                        ErrorCode::InferenceFailed,
                        "worker progress values are invalid",
                    ));
                }
                on_jsonl(line)
            }
            "completed" if state == 3 => {
                if event
                    .pointer("/payload/outputPath")
                    .and_then(serde_json::Value::as_str)
                    .map(PathBuf::from)
                    .is_none_or(|path| !same_path(&path, vocals))
                {
                    return Err(AppError::new(
                        ErrorCode::InferenceFailed,
                        "worker completed output path mismatch",
                    ));
                }
                state = 4
            }
            "error" => {
                return Err(worker_error(
                    event
                        .pointer("/payload/code")
                        .and_then(serde_json::Value::as_str),
                ));
            }
            _ => {
                return Err(AppError::new(
                    ErrorCode::InferenceFailed,
                    "worker lifecycle order is invalid",
                ));
            }
        }
    }
    if state == 4 && vocals.is_file() {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::InferenceFailed,
            "worker did not complete vocals output",
        ))
    }
}
fn same_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .replace('/', "\\")
        .eq_ignore_ascii_case(&right.to_string_lossy().replace('/', "\\"))
}
fn publish_partial(partial: &Path, final_path: &Path, overwrite: bool) -> Result<(), AppError> {
    if !final_path.exists() {
        return fs::rename(partial, final_path).map_err(job_error);
    }
    if !overwrite {
        return Err(AppError::new(
            ErrorCode::OutputNotWritable,
            "final output already exists",
        ));
    }
    let final_w: Vec<u16> = final_path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let partial_w: Vec<u16> = partial.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe {
        ReplaceFileW(
            final_w.as_ptr(),
            partial_w.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    } == 0
    {
        let _ = fs::remove_file(partial);
        return Err(job_error(std::io::Error::last_os_error()));
    }
    Ok(())
}
fn worker_error(code: Option<&str>) -> AppError {
    let code = match code {
        Some("CUDA_NOT_AVAILABLE") => ErrorCode::CudaNotAvailable,
        Some("CUDA_OUT_OF_MEMORY") => ErrorCode::CudaOutOfMemory,
        Some("TASK_CANCELLED") => ErrorCode::TaskCancelled,
        _ => ErrorCode::InferenceFailed,
    };
    AppError::new(code, "worker reported a stable failure code")
}

#[derive(Clone, Debug)]
pub enum BatchRunnerEvent {
    Progress(BatchProgress),
    ItemCompleted(BatchItemResult),
    Completed(BatchResult),
    Failed(AppError),
    Cancelled,
}

pub struct SequentialBatchRunner {
    pub app_paths: AppPaths,
    pub runtime: BatchRuntime,
    pub executor: Arc<dyn BatchExecutor>,
}
impl SequentialBatchRunner {
    pub fn run(
        &self,
        task_id: &str,
        plan: BatchPlan,
        cancellation: &CancellationToken,
        mut emit: impl FnMut(BatchRunnerEvent),
    ) -> BatchResult {
        let started = Instant::now();
        let mut result = BatchResult {
            task_id: task_id.into(),
            output_directory: readable_path(&plan.output_directory),
            succeeded: 0,
            failed: plan.preflight_failures.len() as u32,
            skipped: plan.skipped as u32,
            cancelled: false,
            items: plan.preflight_failures.clone(),
        };
        for failure in &plan.preflight_failures {
            emit(BatchRunnerEvent::ItemCompleted(failure.clone()));
        }
        if let Err(error) = self.runtime.validate() {
            emit(BatchRunnerEvent::Failed(error));
            return result;
        }
        let mut completed_duration = 0.0;
        for (index, item) in plan.items.iter().enumerate() {
            if cancellation.is_cancelled() {
                result.cancelled = true;
                emit(BatchRunnerEvent::Cancelled);
                break;
            }
            let job =
                match create_job_paths(&self.app_paths, task_id, &format!("item-{}", index + 1)) {
                    Ok(paths) => paths,
                    Err(error) => {
                        result.failed += 1;
                        emit(BatchRunnerEvent::ItemCompleted(BatchItemResult {
                            item_index: item.item_index,
                            input_path: readable_path(&item.input.absolute_path),
                            outputs: Vec::new(),
                            duration_seconds: item.source_info.duration_seconds,
                            warnings: Vec::new(),
                            error_code: Some(error.code),
                        }));
                        continue;
                    }
                };
            let progress_context = ProgressContext {
                item_count: plan.total_input_count,
                item,
                completed_duration,
                total_duration: plan.total_duration_seconds,
                started,
            };
            emit_progress(
                &mut emit,
                &progress_context,
                BatchStage::PreparingInput,
                0.0,
            );
            let mut processed = self
                .executor
                .prepare_model_input(&self.runtime, item, &job, cancellation)
                .and_then(|_| {
                    emit_progress(&mut emit, &progress_context, BatchStage::Separating, 0.0);
                    self.executor.run_worker_direct(
                        task_id,
                        &self.runtime,
                        item,
                        &job,
                        cancellation,
                        &mut |line| {
                            if let Ok(worker) = parse_worker_progress(line) {
                                let fraction = worker
                                    .current
                                    .zip(worker.total)
                                    .and_then(|(current, total)| {
                                        (total > 0).then_some(current as f64 / total as f64)
                                    })
                                    .unwrap_or(0.0);
                                emit_progress(
                                    &mut emit,
                                    &progress_context,
                                    BatchStage::Separating,
                                    fraction,
                                );
                            }
                        },
                    )
                })
                .and_then(|_| {
                    let outputs = item
                        .outputs
                        .iter()
                        .filter(|output| !output.skip)
                        .collect::<Vec<_>>();
                    for (output_index, output) in outputs.iter().enumerate() {
                        let (start, build_end, end) =
                            postprocess_boundaries(output_index, outputs.len());
                        emit_postprocess_progress(
                            &mut emit,
                            &progress_context,
                            match output.mode {
                                ProcessingMode::Compatibility44100 => {
                                    BatchStage::BuildingCompatibilityOutput
                                }
                                ProcessingMode::SourceSampleRate => {
                                    BatchStage::BuildingSourceRateOutput
                                }
                            },
                            0.0,
                            start,
                        );
                        self.executor.build_output(
                            &self.runtime,
                            item,
                            &job,
                            output,
                            cancellation,
                        )?;
                        emit_postprocess_progress(
                            &mut emit,
                            &progress_context,
                            match output.mode {
                                ProcessingMode::Compatibility44100 => {
                                    BatchStage::BuildingCompatibilityOutput
                                }
                                ProcessingMode::SourceSampleRate => {
                                    BatchStage::BuildingSourceRateOutput
                                }
                            },
                            1.0,
                            build_end,
                        );
                        emit_postprocess_progress(
                            &mut emit,
                            &progress_context,
                            BatchStage::ValidatingOutput,
                            0.0,
                            build_end,
                        );
                        self.executor.validate_and_publish(
                            &self.runtime,
                            item,
                            output,
                            cancellation,
                        )?;
                        emit_postprocess_progress(
                            &mut emit,
                            &progress_context,
                            BatchStage::ValidatingOutput,
                            1.0,
                            end,
                        );
                    }
                    Ok(())
                });
            if cancellation.is_cancelled() {
                processed = Err(AppError::new(
                    ErrorCode::TaskCancelled,
                    "batch cancellation accepted",
                ));
            }
            match processed {
                Ok(()) => {
                    let outputs = item
                        .outputs
                        .iter()
                        .filter(|output| !output.skip)
                        .map(|output| output.final_path.display().to_string())
                        .collect();
                    result.succeeded += 1;
                    result.items.push(BatchItemResult {
                        item_index: item.item_index,
                        input_path: readable_path(&item.input.absolute_path),
                        outputs,
                        duration_seconds: item.source_info.duration_seconds,
                        warnings: Vec::new(),
                        error_code: None,
                    });
                    emit(BatchRunnerEvent::ItemCompleted(
                        result.items.last().unwrap().clone(),
                    ));
                    completed_duration += item.source_info.duration_seconds;
                    emit_progress(
                        &mut emit,
                        &ProgressContext {
                            completed_duration,
                            ..progress_context
                        },
                        BatchStage::CleaningUp,
                        1.0,
                    );
                }
                Err(error) if error.code == ErrorCode::TaskCancelled => {
                    result.cancelled = true;
                    emit(BatchRunnerEvent::Cancelled);
                }
                Err(error) => {
                    result.failed += 1;
                    let failed_item = BatchItemResult {
                        item_index: item.item_index,
                        input_path: readable_path(&item.input.absolute_path),
                        outputs: Vec::new(),
                        duration_seconds: item.source_info.duration_seconds,
                        warnings: Vec::new(),
                        error_code: Some(error.code),
                    };
                    result.items.push(failed_item.clone());
                    emit(BatchRunnerEvent::ItemCompleted(failed_item));
                    completed_duration += item.source_info.duration_seconds;
                    emit_progress(
                        &mut emit,
                        &ProgressContext {
                            completed_duration,
                            ..progress_context
                        },
                        BatchStage::CleaningUp,
                        1.0,
                    );
                }
            }
            for output in &item.outputs {
                let _ = fs::remove_file(&output.partial_path);
            }
            let _ = cleanup_job(&self.app_paths, &job);
            if result.cancelled {
                break;
            }
        }
        emit(BatchRunnerEvent::Completed(result.clone()));
        result
    }
}

#[derive(Clone, Copy)]
struct ProgressContext<'a> {
    item_count: usize,
    item: &'a PlannedItem,
    completed_duration: f64,
    total_duration: f64,
    started: Instant,
}
fn emit_progress(
    emit: &mut impl FnMut(BatchRunnerEvent),
    context: &ProgressContext<'_>,
    stage: BatchStage,
    current_fraction: f64,
) {
    let stage_fraction = match stage {
        BatchStage::Probing => current_fraction * 0.01,
        BatchStage::PreparingInput => 0.01 + current_fraction * 0.04,
        BatchStage::Separating => 0.05 + current_fraction * 0.85,
        BatchStage::BuildingCompatibilityOutput | BatchStage::BuildingSourceRateOutput => {
            0.90 + current_fraction * 0.07
        }
        BatchStage::ValidatingOutput => 0.97 + current_fraction * 0.02,
        BatchStage::CleaningUp => 0.99 + current_fraction * 0.01,
    };
    let overall = duration_weighted_fraction(
        context.completed_duration,
        context.item.source_info.duration_seconds,
        stage_fraction,
        context.total_duration,
    )
    .unwrap_or(0.0);
    emit(BatchRunnerEvent::Progress(BatchProgress {
        item_index: context.item.item_index,
        item_count: context.item_count as u32,
        current_input_path: readable_path(&context.item.input.absolute_path),
        current_display_name: context.item.input.relative_path.display().to_string(),
        stage,
        overall: ProgressValue::Determinate { fraction: overall },
        current: ProgressValue::Determinate {
            fraction: current_fraction.clamp(0.0, 1.0),
        },
        completed_duration_seconds: context.completed_duration,
        total_duration_seconds: context.total_duration,
        elapsed_seconds: context.started.elapsed().as_secs_f64(),
    }));
}
fn emit_postprocess_progress(
    emit: &mut impl FnMut(BatchRunnerEvent),
    context: &ProgressContext<'_>,
    stage: BatchStage,
    current_fraction: f64,
    combined_fraction: f64,
) {
    let overall = duration_weighted_fraction(
        context.completed_duration,
        context.item.source_info.duration_seconds,
        0.90 + combined_fraction.clamp(0.0, 1.0) * 0.09,
        context.total_duration,
    )
    .unwrap_or(0.0);
    emit(BatchRunnerEvent::Progress(BatchProgress {
        item_index: context.item.item_index,
        item_count: context.item_count as u32,
        current_input_path: readable_path(&context.item.input.absolute_path),
        current_display_name: context.item.input.relative_path.display().to_string(),
        stage,
        overall: ProgressValue::Determinate { fraction: overall },
        current: ProgressValue::Determinate {
            fraction: current_fraction.clamp(0.0, 1.0),
        },
        completed_duration_seconds: context.completed_duration,
        total_duration_seconds: context.total_duration,
        elapsed_seconds: context.started.elapsed().as_secs_f64(),
    }));
}

/// Returns the combined post-processing progress boundaries for one output.
/// Building consumes 7/9 of each output slice (the batch's 0.07 weight) and
/// validation/publishing consumes the remaining 2/9 (the batch's 0.02 weight).
fn postprocess_boundaries(output_index: usize, output_count: usize) -> (f64, f64, f64) {
    debug_assert!(output_count > 0);
    let count = output_count.max(1) as f64;
    let start = output_index as f64 / count;
    let end = (output_index + 1) as f64 / count;
    let build_end = start + (end - start) * (7.0 / 9.0);
    (start, build_end, end)
}

fn job_error(_: impl std::fmt::Display) -> AppError {
    AppError::new(
        ErrorCode::InputUnsupported,
        "input planning filesystem operation failed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::Mutex};
    use uuid::Uuid;
    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("soufmer-jobs-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn request(root: &Path, output: &Path) -> StartBatchRequest {
        StartBatchRequest {
            input_mode: InputMode::Folder,
            input_path: root.display().to_string(),
            output_directory: output.display().to_string(),
            processing_mode: ProcessingMode::Compatibility44100,
            recursive: true,
            preserve_directory_structure: false,
            conflict_policy: ConflictPolicy::Skip,
            output_format: OutputFormat::Flac,
            generate_both_modes: false,
        }
    }
    #[test]
    fn enumeration_is_sorted_and_excludes_output_subtree() {
        let root = temp_dir();
        let output = root.join("output");
        fs::create_dir_all(root.join("b")).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(root.join("b").join("z.flac"), b"x").unwrap();
        fs::write(root.join("a.wav"), b"x").unwrap();
        fs::write(output.join("generated.flac"), b"x").unwrap();
        let paths = enumerate_inputs(&request(&root, &output)).unwrap();
        assert_eq!(
            paths
                .iter()
                .map(|input| input.relative_path.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["a.wav", "b\\z.flac"]
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn generated_outputs_are_excluded() {
        let root = temp_dir();
        let output = root.join("output");
        fs::create_dir_all(&output).unwrap();
        fs::write(root.join("song (Instrumental).flac"), b"x").unwrap();
        assert!(
            enumerate_inputs(&request(&root, &output))
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn generated_output_exclusion_is_case_insensitive() {
        assert!(is_generated_output(Path::new(
            "SONG (INSTRUMENTAL - 44.1K).flac"
        )));
    }
    #[test]
    fn conflict_policy_sets_explicit_publication_flags() {
        let root = temp_dir();
        let input = root.join("song.flac");
        fs::write(&input, []).unwrap();
        let existing = root.join("song (Instrumental).flac");
        fs::write(&existing, []).unwrap();
        let skip = plan_one_output(
            &root,
            &input,
            "task",
            OutputFormat::Flac,
            ConflictPolicy::Skip,
            ProcessingMode::Compatibility44100,
            false,
        )
        .unwrap();
        assert!(skip.skip && !skip.overwrite);
        let overwrite = plan_one_output(
            &root,
            &input,
            "task",
            OutputFormat::Flac,
            ConflictPolicy::Overwrite,
            ProcessingMode::Compatibility44100,
            false,
        )
        .unwrap();
        assert!(!overwrite.skip && overwrite.overwrite && overwrite.final_path == existing);
        let numbered = plan_one_output(
            &root,
            &input,
            "task",
            OutputFormat::Flac,
            ConflictPolicy::AutoNumber,
            ProcessingMode::Compatibility44100,
            false,
        )
        .unwrap();
        assert!(
            !numbered.skip
                && !numbered.overwrite
                && numbered
                    .final_path
                    .ends_with("song (Instrumental) (2).flac")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn worker_progress_requires_versioned_envelope() {
        let progress = parse_worker_progress(r#"{"schemaVersion":1,"type":"progress","taskId":"00000000-0000-0000-0000-000000000000","payload":{"current":3,"total":4}}"#).unwrap();
        assert_eq!((progress.current, progress.total), (Some(3), Some(4)));
        assert!(parse_worker_progress(r#"{"stage":"separating"}"#).is_err());
    }
    struct FakeExecutor {
        prepared: Mutex<Vec<String>>,
        fail_second: bool,
    }
    impl BatchExecutor for FakeExecutor {
        fn prepare_model_input(
            &self,
            _: &BatchRuntime,
            item: &PlannedItem,
            _: &JobPaths,
            _: &CancellationToken,
        ) -> Result<(), AppError> {
            self.prepared
                .lock()
                .unwrap()
                .push(item.input.relative_path.display().to_string());
            if self.fail_second && item.input.relative_path == Path::new("second.flac") {
                return Err(AppError::new(
                    ErrorCode::InferenceFailed,
                    "fake item failure",
                ));
            }
            Ok(())
        }
        fn run_worker_direct(
            &self,
            _: &str,
            _: &BatchRuntime,
            _: &PlannedItem,
            _: &JobPaths,
            _: &CancellationToken,
            callback: &mut dyn FnMut(&str),
        ) -> Result<(), AppError> {
            callback(
                r#"{"schemaVersion":1,"type":"progress","taskId":"00000000-0000-0000-0000-000000000000","payload":{"current":1,"total":2}}"#,
            );
            Ok(())
        }
        fn build_output(
            &self,
            _: &BatchRuntime,
            _: &PlannedItem,
            _: &JobPaths,
            _: &PlannedOutput,
            _: &CancellationToken,
        ) -> Result<(), AppError> {
            Ok(())
        }
        fn validate_and_publish(
            &self,
            _: &BatchRuntime,
            _: &PlannedItem,
            _: &PlannedOutput,
            _: &CancellationToken,
        ) -> Result<(), AppError> {
            Ok(())
        }
    }
    fn planned(name: &str, duration: f64) -> PlannedItem {
        PlannedItem {
            item_index: 1,
            input: EnumeratedInput {
                absolute_path: PathBuf::from(name),
                relative_path: PathBuf::from(name),
            },
            source_info: AudioSourceInfo {
                stream_index: 0,
                codec_name: "flac".into(),
                sample_rate: 44_100,
                channels: 2,
                duration_seconds: duration,
                sample_format: None,
                bits_per_sample: None,
                bits_per_raw_sample: None,
                container_format: None,
            },
            outputs: vec![PlannedOutput {
                mode: ProcessingMode::Compatibility44100,
                final_path: PathBuf::from(format!("{name}.out")),
                partial_path: PathBuf::from(format!("{name}.partial")),
                skip: false,
                overwrite: false,
                format: OutputFormat::Flac,
            }],
        }
    }
    fn runtime(root: &Path) -> BatchRuntime {
        let ffmpeg = root.join("ffmpeg.exe");
        let ffprobe = root.join("ffprobe.exe");
        let python = root.join("python.exe");
        let checkpoint = root.join("model.ckpt");
        let config = root.join("config.yaml");
        let worker_cwd = root.join("worker");
        fs::write(&ffmpeg, []).unwrap();
        fs::write(&ffprobe, []).unwrap();
        fs::write(&python, []).unwrap();
        fs::write(&checkpoint, []).unwrap();
        fs::write(&config, []).unwrap();
        fs::create_dir(&worker_cwd).unwrap();
        BatchRuntime {
            ffmpeg,
            ffprobe,
            python,
            worker_module: "accompaniment_worker".into(),
            worker_cwd,
            checkpoint,
            config,
            environment: Vec::new(),
            logs: root.join("logs"),
        }
    }

    #[test]
    fn worker_process_is_isolated_with_exact_args_and_job_temp() {
        let root = temp_dir();
        let app_paths = AppPaths::from_test_root(root.join("private"));
        let paths =
            create_job_paths(&app_paths, "00000000-0000-0000-0000-000000000000", "item-1").unwrap();
        let request = paths.root.join("request.json");
        let mut runtime = runtime(&root);
        runtime.environment = vec![
            ("TEMP".into(), root.join("old-temp").into_os_string()),
            ("tmp".into(), root.join("old-tmp").into_os_string()),
            (
                "HF_HOME".into(),
                root.join("private-cache").into_os_string(),
            ),
            ("PYTHONNOUSERSITE".into(), "1".into()),
        ];

        let spec = worker_process_spec(&runtime, &paths, &request).unwrap();
        assert_eq!(spec.executable, runtime.python);
        assert_eq!(
            spec.arguments,
            [
                "-I",
                "-m",
                "accompaniment_worker",
                "separate",
                "--request",
                request.to_str().unwrap(),
            ]
            .into_iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>()
        );
        assert_eq!(spec.current_dir, Some(runtime.worker_cwd));
        assert_eq!(spec.stderr_log, Some(paths.logs.join("worker.stderr.log")));
        let environment = spec
            .environment
            .into_iter()
            .map(|(name, value)| (name.to_string_lossy().to_uppercase(), PathBuf::from(value)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let job_temp = paths.root.join("tmp");
        assert_eq!(environment["TEMP"], job_temp);
        assert_eq!(environment["TMP"], job_temp);
        assert_eq!(environment["HF_HOME"], root.join("private-cache"));
        assert_eq!(environment["PYTHONNOUSERSITE"], PathBuf::from("1"));
        assert!(job_temp.is_dir());
        assert!(job_temp.starts_with(&paths.root));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runner_is_sequential_and_continues_after_an_item_failure() {
        let root = temp_dir();
        let executor = Arc::new(FakeExecutor {
            prepared: Mutex::new(Vec::new()),
            fail_second: true,
        });
        let runner = SequentialBatchRunner {
            app_paths: AppPaths::from_test_root(root.clone()),
            runtime: runtime(&root),
            executor: executor.clone(),
        };
        let plan = BatchPlan {
            items: vec![
                planned("first.flac", 2.0),
                planned("second.flac", 3.0),
                planned("third.flac", 5.0),
            ],
            preflight_failures: Vec::new(),
            output_directory: root.clone(),
            total_duration_seconds: 10.0,
            skipped: 0,
            total_input_count: 3,
        };
        let result = runner.run("task-1", plan, &CancellationToken::new(), |_| {});
        assert_eq!(
            *executor.prepared.lock().unwrap(),
            vec!["first.flac", "second.flac", "third.flac"]
        );
        assert_eq!((result.succeeded, result.failed), (2, 1));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn runner_preserves_cancellation_as_terminal_batch_state() {
        let root = temp_dir();
        let executor = Arc::new(FakeExecutor {
            prepared: Mutex::new(Vec::new()),
            fail_second: false,
        });
        let runner = SequentialBatchRunner {
            app_paths: AppPaths::from_test_root(root.clone()),
            runtime: runtime(&root),
            executor,
        };
        let token = CancellationToken::new();
        token.cancel();
        let result = runner.run(
            "task-2",
            BatchPlan {
                items: vec![planned("first.flac", 1.0)],
                preflight_failures: Vec::new(),
                output_directory: root.clone(),
                total_duration_seconds: 1.0,
                skipped: 0,
                total_input_count: 1,
            },
            &token,
            |_| {},
        );
        assert!(result.cancelled);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn postprocess_boundaries_honor_build_and_validation_weights() {
        assert_eq!(postprocess_boundaries(0, 1), (0.0, 7.0 / 9.0, 1.0));
        assert_eq!(postprocess_boundaries(0, 2), (0.0, 7.0 / 18.0, 0.5));
        assert_eq!(postprocess_boundaries(1, 2), (0.5, 8.0 / 9.0, 1.0));
    }
    #[test]
    fn one_and_two_output_postprocess_progress_is_monotonic_and_ordered() {
        let root = temp_dir();
        let executor = Arc::new(FakeExecutor {
            prepared: Mutex::new(Vec::new()),
            fail_second: false,
        });
        let runner = SequentialBatchRunner {
            app_paths: AppPaths::from_test_root(root.clone()),
            runtime: runtime(&root),
            executor,
        };
        for output_count in [1, 2] {
            let mut item = planned("song.flac", 2.0);
            if output_count == 2 {
                let mut second = item.outputs[0].clone();
                second.mode = ProcessingMode::SourceSampleRate;
                second.final_path = PathBuf::from("song-source.out");
                second.partial_path = PathBuf::from("song-source.partial");
                item.outputs.push(second);
            }
            let mut progress = Vec::new();
            runner.run(
                "00000000-0000-0000-0000-000000000000",
                BatchPlan {
                    items: vec![item],
                    preflight_failures: Vec::new(),
                    output_directory: root.clone(),
                    total_duration_seconds: 2.0,
                    skipped: 0,
                    total_input_count: 1,
                },
                &CancellationToken::new(),
                |event| {
                    if let BatchRunnerEvent::Progress(value) = event {
                        progress.push(value)
                    }
                },
            );
            let fractions = progress
                .iter()
                .map(|value| match value.overall {
                    ProgressValue::Determinate { fraction } => fraction,
                    _ => 0.0,
                })
                .collect::<Vec<_>>();
            assert!(fractions.windows(2).all(|pair| pair[0] <= pair[1]));

            let postprocess = progress
                .iter()
                .filter(|value| {
                    matches!(
                        value.stage,
                        BatchStage::BuildingCompatibilityOutput
                            | BatchStage::BuildingSourceRateOutput
                            | BatchStage::ValidatingOutput
                    )
                })
                .collect::<Vec<_>>();
            let expected_fractions = if output_count == 1 {
                vec![0.90, 0.97, 0.97, 0.99]
            } else {
                vec![0.90, 0.935, 0.935, 0.945, 0.945, 0.98, 0.98, 0.99]
            };
            let expected_stages = if output_count == 1 {
                vec![
                    BatchStage::BuildingCompatibilityOutput,
                    BatchStage::BuildingCompatibilityOutput,
                    BatchStage::ValidatingOutput,
                    BatchStage::ValidatingOutput,
                ]
            } else {
                vec![
                    BatchStage::BuildingCompatibilityOutput,
                    BatchStage::BuildingCompatibilityOutput,
                    BatchStage::ValidatingOutput,
                    BatchStage::ValidatingOutput,
                    BatchStage::BuildingSourceRateOutput,
                    BatchStage::BuildingSourceRateOutput,
                    BatchStage::ValidatingOutput,
                    BatchStage::ValidatingOutput,
                ]
            };
            assert_eq!(postprocess.len(), expected_fractions.len());
            for ((value, expected_fraction), expected_stage) in postprocess
                .iter()
                .zip(expected_fractions)
                .zip(expected_stages)
            {
                let ProgressValue::Determinate { fraction } = value.overall else {
                    panic!("post-processing progress must be determinate");
                };
                assert!((fraction - expected_fraction).abs() < 1e-12);
                assert_eq!(
                    std::mem::discriminant(&value.stage),
                    std::mem::discriminant(&expected_stage)
                );
            }
            assert_eq!(
                postprocess
                    .iter()
                    .map(|value| match value.current {
                        ProgressValue::Determinate { fraction } => fraction,
                        _ => -1.0,
                    })
                    .collect::<Vec<_>>(),
                (0..output_count)
                    .flat_map(|_| [0.0, 1.0, 0.0, 1.0])
                    .collect::<Vec<_>>()
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn worker_request_and_lifecycle_are_strict() {
        let root = temp_dir();
        let input = root.join("input.wav");
        let vocals = root.join("vocals.wav");
        let checkpoint = root.join("model.ckpt");
        let config = root.join("config.yaml");
        for path in [&input, &vocals, &checkpoint, &config] {
            fs::write(path, []).unwrap();
        }
        let task = "00000000-0000-0000-0000-000000000000";
        let request =
            super::worker_request_json(task, &input, &vocals, &checkpoint, &config).unwrap();
        assert_eq!(request.as_object().unwrap().len(), 9);
        assert_eq!(request["device"], "cuda:0");
        assert_eq!(request["batchSize"], 1);
        assert_eq!(request["overlap"], 4);
        for field in [
            "inputPath",
            "outputVocalsPath",
            "checkpointPath",
            "configPath",
        ] {
            let value = request[field].as_str().unwrap();
            assert!(Path::new(value).is_absolute());
            assert!(!value.starts_with(r"\\?\"));
        }
        let ready = format!(
            r#"{{"schemaVersion":1,"type":"ready","taskId":"{task}","payload":{{"device":"cuda:0"}}}}"#
        );
        let loading = format!(
            r#"{{"schemaVersion":1,"type":"stage","taskId":"{task}","payload":{{"stage":"loadingModel"}}}}"#
        );
        let separating = format!(
            r#"{{"schemaVersion":1,"type":"stage","taskId":"{task}","payload":{{"stage":"separating"}}}}"#
        );
        let completed = format!(
            r#"{{"schemaVersion":1,"type":"completed","taskId":"{task}","payload":{{"outputPath":{:?}}}}}"#,
            vocals.to_string_lossy()
        );
        let lines = vec![
            ready.clone(),
            loading.clone(),
            separating.clone(),
            completed.clone(),
        ];
        assert!(super::validate_worker_lifecycle(&lines, task, &vocals, &mut |_| {}).is_ok());
        assert!(
            super::validate_worker_lifecycle(
                &[ready.clone(), loading.clone(), separating.clone()],
                task,
                &vocals,
                &mut |_| {}
            )
            .is_err()
        );
        assert!(
            super::validate_worker_lifecycle(
                &[
                    ready.clone(),
                    loading.clone(),
                    separating.clone(),
                    completed.clone(),
                    completed.clone()
                ],
                task,
                &vocals,
                &mut |_| {}
            )
            .is_err()
        );
        let wrong_completed = serde_json::json!({"schemaVersion":1,"type":"completed","taskId":task,"payload":{"outputPath":"C:\\wrong.wav"}}).to_string();
        assert!(
            super::validate_worker_lifecycle(
                &[
                    ready.clone(),
                    loading.clone(),
                    separating.clone(),
                    wrong_completed
                ],
                task,
                &vocals,
                &mut |_| {}
            )
            .is_err()
        );
        assert!(
            super::validate_worker_lifecycle(&["{".into()], task, &vocals, &mut |_| {}).is_err()
        );
        assert!(
            super::validate_worker_lifecycle(
                std::slice::from_ref(&loading),
                task,
                &vocals,
                &mut |_| {}
            )
            .is_err()
        );
        assert!(
            super::validate_worker_lifecycle(
                &[
                    ready,
                    loading,
                    separating,
                    completed.replace(task, "11111111-1111-1111-1111-111111111111")
                ],
                task,
                &vocals,
                &mut |_| {}
            )
            .is_err()
        );
        assert_eq!(
            super::worker_error(Some("CUDA_NOT_AVAILABLE")).code,
            ErrorCode::CudaNotAvailable
        );
        assert_eq!(
            super::worker_error(Some("CUDA_OUT_OF_MEMORY")).code,
            ErrorCode::CudaOutOfMemory
        );
        assert_eq!(
            super::worker_error(Some("OTHER")).code,
            ErrorCode::InferenceFailed
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires the initialized private CUDA runtime and a real NVIDIA GPU"]
    fn real_private_runtime_inference_publishes_local_output() {
        let app_paths = AppPaths::discover().unwrap();
        let runtime = BatchRuntime::resolve(&app_paths).unwrap();
        let root = temp_dir().join("GPU smoke 简体中文");
        let input = root.join("input with spaces").join("简体中文 signal.wav");
        let output = root.join("published output");
        fs::create_dir_all(input.parent().unwrap()).unwrap();
        fs::create_dir_all(&output).unwrap();
        let mut fixture = ProcessSpec::new(&runtime.ffmpeg);
        fixture.arguments = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-y".into(),
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "sine=frequency=440:duration=1:sample_rate=44100".into(),
            "-filter_complex".into(),
            "[0:a]pan=stereo|c0=c0|c1=c0[out]".into(),
            "-map".into(),
            "[out]".into(),
            "-c:a".into(),
            "pcm_f32le".into(),
            external_process_path(&input).unwrap().into_os_string(),
        ];
        fixture.current_dir = Some(root.clone());
        let fixture_result =
            ProcessRunner::run(fixture, CancellationToken::new(), Arc::new(|_| {})).unwrap();
        assert_eq!(fixture_result.exit_code, Some(0));

        let task_id = Uuid::new_v4().to_string();
        let request = StartBatchRequest {
            input_mode: InputMode::File,
            input_path: input.display().to_string(),
            output_directory: output.display().to_string(),
            processing_mode: ProcessingMode::Compatibility44100,
            recursive: false,
            preserve_directory_structure: false,
            conflict_policy: ConflictPolicy::Overwrite,
            output_format: OutputFormat::WavFloat32,
            generate_both_modes: false,
        };
        let prober = FfprobeInputProber {
            ffprobe: runtime.ffprobe.clone(),
            logs: app_paths.logs(),
            cancellation: CancellationToken::new(),
        };
        let plan = preflight_plan(&request, &task_id, &prober).unwrap();
        assert!(
            !plan.items.is_empty(),
            "preflight failures: {:?}",
            plan.preflight_failures
                .iter()
                .map(|item| item.error_code)
                .collect::<Vec<_>>()
        );
        let expected = plan.items[0].outputs[0].final_path.clone();
        let runner = SequentialBatchRunner {
            app_paths,
            runtime,
            executor: Arc::new(ProductionBatchExecutor),
        };
        let result = runner.run(&task_id, plan, &CancellationToken::new(), |_| {});
        assert_eq!(
            (result.succeeded, result.failed, result.cancelled),
            (1, 0, false)
        );
        assert!(expected.is_file());
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }
}
