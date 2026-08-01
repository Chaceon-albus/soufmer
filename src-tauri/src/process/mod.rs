use std::{
    collections::VecDeque,
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::windows::{io::AsRawHandle, process::CommandExt},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    },
};

use crate::domain::{AppError, ErrorCode};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const INHERITED_CONFIGURATION_VARIABLES: &[&str] = &[
    "PYTHONHOME",
    "PYTHONPATH",
    "PYTHONSTARTUP",
    "PYTHONUSERBASE",
    "PYTHONSAFEPATH",
    "VIRTUAL_ENV",
    "UV_CONFIG_FILE",
    "UV_PROJECT",
    "UV_WORKSPACE",
    "UV_PYTHON",
    "UV_SYSTEM_PYTHON",
];

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug)]
pub struct ProcessSpec {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    pub environment: Vec<(OsString, OsString)>,
    pub stderr_limit_bytes: usize,
    pub stderr_log: Option<PathBuf>,
}

impl ProcessSpec {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            arguments: Vec::new(),
            current_dir: None,
            environment: Vec::new(),
            stderr_limit_bytes: 256 * 1024,
            stderr_log: None,
        }
    }
    pub fn arg(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }
}

#[derive(Debug)]
pub struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout_line_count: usize,
    pub stdout_lines: Vec<String>,
    pub stderr: String,
    pub stderr_truncated: bool,
}

pub struct ProcessRunner;

impl ProcessRunner {
    pub fn run(
        spec: ProcessSpec,
        cancellation: CancellationToken,
        on_stdout_line: Arc<dyn Fn(String) + Send + Sync>,
    ) -> Result<ProcessOutput, AppError> {
        if cancellation.is_cancelled() {
            return Err(AppError::new(
                ErrorCode::TaskCancelled,
                "process start was cancelled",
            ));
        }
        let stderr_log = if let Some(log_path) = &spec.stderr_log {
            if let Some(parent) = log_path.parent() {
                std::fs::create_dir_all(parent).map_err(|_| {
                    AppError::new(
                        ErrorCode::LocalDataUnavailable,
                        "could not create process log directory",
                    )
                })?;
            }
            Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                    .map_err(|_| {
                        AppError::new(
                            ErrorCode::LocalDataUnavailable,
                            "could not open process stderr log",
                        )
                    })?,
            )
        } else {
            None
        };
        let job = JobObject::new()?;
        let mut command = Command::new(&spec.executable);
        command
            .args(&spec.arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        if let Some(current_dir) = &spec.current_dir {
            command.current_dir(current_dir);
        }
        command.envs(spec.environment.iter().cloned());
        // Preserve normal proxy settings, but do not allow a parent Python or uv configuration
        // to redirect the private runtime's controlled commands.
        for name in INHERITED_CONFIGURATION_VARIABLES {
            command.env_remove(name);
        }
        command.env("UV_NO_CONFIG", "1");
        command.creation_flags(CREATE_NO_WINDOW);
        let mut child = command.spawn().map_err(|_| {
            AppError::new(
                ErrorCode::InferenceFailed,
                "could not start controlled child process",
            )
        })?;
        if let Err(error) = job.assign(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }

        let stdout_lines = Arc::new(std::sync::Mutex::new(Vec::new()));
        let stdout_capture = Arc::clone(&stdout_lines);
        let external_callback = Arc::clone(&on_stdout_line);
        let combined_callback: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |line| {
            stdout_capture
                .lock()
                .expect("stdout capture lock poisoned")
                .push(line.clone());
            external_callback(line);
        });
        let stdout = child.stdout.take().expect("stdout pipe configured");
        let stderr = child.stderr.take().expect("stderr pipe configured");
        let stdout_handle = thread::spawn(move || read_stdout_lines(stdout, combined_callback));
        let stderr_limit = spec.stderr_limit_bytes;
        let stderr_handle =
            thread::spawn(move || read_bounded_stderr(stderr, stderr_limit, stderr_log));
        let exit_code = wait_for_child_or_cancel(&mut child, &job, &cancellation)?;
        let stdout_line_count = stdout_handle.join().map_err(|_| {
            AppError::new(
                ErrorCode::InferenceFailed,
                "stdout reader stopped unexpectedly",
            )
        })?;
        let (stderr, stderr_truncated) = stderr_handle
            .join()
            .map_err(|_| {
                AppError::new(
                    ErrorCode::InferenceFailed,
                    "stderr reader stopped unexpectedly",
                )
            })?
            .map_err(|_| {
                AppError::new(
                    ErrorCode::LocalDataUnavailable,
                    "could not persist process stderr log",
                )
            })?;
        Ok(ProcessOutput {
            exit_code,
            stdout_line_count,
            stdout_lines: stdout_lines
                .lock()
                .expect("stdout capture lock poisoned")
                .clone(),
            stderr,
            stderr_truncated,
        })
    }
}

fn wait_for_child_or_cancel(
    child: &mut Child,
    job: &JobObject,
    cancellation: &CancellationToken,
) -> Result<Option<i32>, AppError> {
    loop {
        if cancellation.is_cancelled() {
            job.terminate()?;
            let _ = child.wait();
            return Err(AppError::new(
                ErrorCode::TaskCancelled,
                "controlled process tree was cancelled",
            ));
        }
        if let Some(status) = child.try_wait().map_err(|_| {
            AppError::new(
                ErrorCode::InferenceFailed,
                "could not wait for child process",
            )
        })? {
            return Ok(status.code());
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn read_stdout_lines(stdout: impl Read, callback: Arc<dyn Fn(String) + Send + Sync>) -> usize {
    let mut count = 0;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        callback(line);
        count += 1;
    }
    count
}

fn read_bounded_stderr(
    stderr: impl Read,
    limit: usize,
    mut log: Option<File>,
) -> std::io::Result<(String, bool)> {
    let mut bounded = VecDeque::with_capacity(limit.min(64 * 1024));
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];
    let mut source = stderr;
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if let Some(log) = log.as_mut() {
            log.write_all(&buffer[..read])?;
        }
        for byte in &buffer[..read] {
            if bounded.len() == limit {
                bounded.pop_front();
                truncated = true;
            }
            if limit > 0 {
                bounded.push_back(*byte);
            }
        }
    }
    Ok((
        String::from_utf8_lossy(bounded.make_contiguous()).into_owned(),
        truncated,
    ))
}

struct JobObject(HANDLE);

impl JobObject {
    fn new() -> Result<Self, AppError> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(AppError::new(
                ErrorCode::InferenceFailed,
                "could not create Windows Job Object",
            ));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            unsafe {
                CloseHandle(handle);
            }
            return Err(AppError::new(
                ErrorCode::InferenceFailed,
                "could not configure Windows Job Object limits",
            ));
        }
        Ok(Self(handle))
    }
    fn assign(&self, child: &Child) -> Result<(), AppError> {
        let assigned = unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle().cast()) };
        if assigned == 0 {
            return Err(AppError::new(
                ErrorCode::InferenceFailed,
                "could not assign child to Windows Job Object",
            ));
        }
        Ok(())
    }
    fn terminate(&self) -> Result<(), AppError> {
        let terminated = unsafe { TerminateJobObject(self.0, 1) };
        if terminated == 0 {
            return Err(AppError::new(
                ErrorCode::TaskCancelled,
                "could not terminate controlled process tree",
            ));
        }
        Ok(())
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CancellationToken, ProcessRunner, ProcessSpec};
    use std::{path::PathBuf, sync::Arc, thread, time::Duration};

    #[test]
    fn cancellation_terminates_a_controlled_child() {
        let token = CancellationToken::new();
        let cancelled = token.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            cancelled.cancel();
        });
        let ping = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("ping.exe");
        let spec = ProcessSpec::new(ping).arg("-n").arg("20").arg("127.0.0.1");
        let error = ProcessRunner::run(spec, token, Arc::new(|_| {})).unwrap_err();
        assert_eq!(error.code, crate::domain::ErrorCode::TaskCancelled);
    }

    #[test]
    fn controlled_children_remove_parent_python_and_uv_configuration() {
        assert!(super::INHERITED_CONFIGURATION_VARIABLES.contains(&"PYTHONPATH"));
        assert!(super::INHERITED_CONFIGURATION_VARIABLES.contains(&"UV_CONFIG_FILE"));
        assert!(!super::INHERITED_CONFIGURATION_VARIABLES.contains(&"HTTPS_PROXY"));
    }
}
