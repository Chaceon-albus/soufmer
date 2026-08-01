# Architecture

soufmer is a Windows-only Tauri 2 desktop application. The frontend presents a localized,
non-technical workflow; Rust owns every trusted filesystem, process, download, and task decision;
and a private Python worker performs one model inference per invocation.

## Component boundaries

```text
React UI
  | narrow typed commands and versioned progress events
Rust/Tauri backend
  |-- private runtime manager (%LOCALAPPDATA%\soufmer)
  |-- sequential job planner and process controller
  |-- FFmpeg/FFprobe argument-array pipelines
  `-- one-file Python worker invocation
Python worker
  `-- pinned MelBandRoformer inference implementation
```

The frontend never constructs commands or receives a generic command API. Rust validates every
IPC request again, launches only validated private executables with argument vectors, and maps
technical failures to stable error codes. Worker stdout is a JSON Lines protocol; stderr and
tracebacks are diagnostic data, not normal user-interface text.

## Application state

The React application uses one discriminated union for the task lifecycle:

```text
idle -> validating -> initializing -> idle
                   `-> failed
idle -> validating -> processing -> completed
                              |  `-> failed
                              `-> cancelling -> completed
```

Persisted preferences are separate from this active state. Rust is authoritative for whether the
environment is ready and whether a task may start.

## Runtime layout

All application-managed writes use the literal Windows known-folder root
`%LOCALAPPDATA%\soufmer\`. Versioned bootstrap and runtime directories are activated through small
state files only after validation and self-test. Downloads, caches, logs, diagnostics, and job
temporary files remain below the same root. User-selected inputs and final outputs are the only
intentional exceptions.

The portable executable embeds a deterministic bootstrap archive. It does not depend on a sibling
resource directory and never writes beside itself. Python, locked packages, FFmpeg, and the model
are installed or downloaded into the private runtime during a confirmed first-run initialization.

## Tasks, progress, and cancellation

Only one initialization or batch task may be active. Each event includes a task ID, monotonically
increasing sequence, timestamp, schema version, event type, and typed payload. The frontend ignores
events for another task or an already-consumed sequence.

Batch planning enumerates and probes every input before inference, excludes the output subtree,
plans conflicts deterministically, and then processes exactly one item at a time. On Windows, every
child process belongs to a Job Object so cancellation terminates the process tree. Completed items
remain published; the current item's partial and temporary files are removed.

## Trust boundary

The compiled bootstrap descriptor and runtime manifest are trusted configuration. Downloaded
bytes and extracted archives are untrusted until their SHA-256 digests, paths, entry types, counts,
and sizes pass validation. Frontend strings, worker messages, media metadata, and user paths are
also untrusted input and are never routed through a command shell.
