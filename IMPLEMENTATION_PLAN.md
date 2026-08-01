# Accompaniment Extractor — Detailed Implementation Plan

Status: Implementation substantially complete; end-to-end stabilization and portable validation in progress
Last reviewed: 2026-08-01
Target platform: Windows 10/11 x64
Default UI locale: Simplified Chinese (`zh-CN`)
Development language: English
Primary stack: Tauri 2, Rust, React, TypeScript, Vite, Tailwind CSS, shadcn/ui, Python managed by uv

## 1. How to use this plan

This plan is written for an implementation agent working inside the repository.

Before starting:

1. Read `AGENTS.md` completely.
2. Work through phases in order.
3. Keep each phase as a coherent, runnable vertical slice.
4. Update the checkboxes when tasks are completed.
5. Record material deviations in the Decision Log at the end of this file.
6. Do not expand the MVP scope without an explicit requirement change.

A phase is complete only when its acceptance criteria and applicable quality gates pass.

This file is the committed `IMPLEMENTATION_PLAN.md`. Update its phase checklists and Decision Log
in the same commit as each significant implementation checkpoint or completed phase.

### Current implementation checkpoint

This checkpoint is based on the committed checklist and the project owner's manual observations. It
is not a substitute for inspecting the current Git worktree, recent commits, diagnostics, and test
results at the start of the next agent run.

- Phases 0 through 15 are implemented according to the checklist, except for the final visual/high-DPI
  polish item in Phase 2 and the real-model GPU inference smoke test in Phase 8.
- The raw standalone executable builds and starts, and first-run private runtime initialization has
  succeeded on the current development machine.
- Most Phase 16 packaging work is complete, including the one-file artifact, but clean-profile,
  prerequisite, concurrency, and write-boundary validation remain open.
- The MVP is not yet end-to-end complete because real audio processing currently fails with
  `INFERENCE_FAILED`. Canonicalized Windows paths are reaching external-process and worker boundaries
  with verbatim prefixes such as `\\?\UNC\server\share\...`.
- Initialization step 4 (`uv sync`) can remain visually indeterminate for a long time without useful
  activity detail, and the main-window layout and color system still need final UX polish.
- Phase 16A is the next priority. Complete it before treating Phase 16 acceptance criteria as passed
  or beginning optional Phase 17 release hardening.

---

## 2. Product summary

Build a desktop GUI that extracts accompaniment from music using the KimberleyJSN MelBandRoformer model through a pinned Music-Source-Separation-Training implementation.

The product is designed for non-technical users:

- The user can copy or move one standalone `soufmer.exe` and run it from any directory.
- The executable performs private runtime initialization on first use; there is no separate
  installer and no required sibling file or directory.
- Command-line windows and command text remain hidden.
- The main screen lets the user select a file or folder, an output directory, and a processing mode.
- A reusable progress dialog shows overall and current-task progress.
- Songs are processed sequentially.
- Temporary files are deleted after each song.

### Processing modes

#### Compatibility mode — default

- Convert the source to 44.1 kHz stereo Float32 with SoXR precision 32.
- Run model inference against that exact file.
- Subtract the 44.1 kHz vocals from the same model-input file.
- Output at 44.1 kHz.

#### Source sample rate mode — experimental

- Convert the vocals output back to the source sample rate with SoXR precision 32.
- Subtract from a Float32 decode of the original source at its native sample rate.
- Align the output sample rate and common bit depths with the source where supported.
- Keep detailed time alignment, resampler-delay correction, and clipping refinement as explicit TODO items.

### Runtime strategy

- Embed a verified uv executable, the worker project, the pinned and patched MSST inference
  snapshot, runtime manifest, configuration, and required notices inside `soufmer.exe`.
- Extract that trusted bootstrap payload only under `%LOCALAPPDATA%\soufmer\`.
- Download FFmpeg/FFprobe, the model checkpoint, Python, and locked Python dependencies during
  first-run initialization.
- Do not install Git.
- Do not clone repositories on the user's computer.
- Do not use the user's global Python environment.
- Keep all application-managed writable state under `%LOCALAPPDATA%\soufmer\`. User-selected input
  and output paths are the deliberate exception; the executable itself may be stored anywhere.

---

## 3. Fixed architectural decisions

These decisions should not be revisited during normal implementation.

| Area | Decision |
|---|---|
| Desktop framework | Tauri 2 |
| Frontend | React + TypeScript + Vite |
| Styling | Tailwind CSS current Vite integration |
| Components | shadcn/ui |
| Icons | Lucide React |
| Localization | i18next + react-i18next |
| Frontend state | `useReducer` with discriminated unions |
| Backend | Rust commands plus typed progress events |
| Worker | Python CLI with JSON request and JSON Lines events |
| Runtime manager | uv embedded in the executable, private `%LOCALAPPDATA%\soufmer` runtime |
| Model | KimberleyJSN MelBandRoformer |
| Inference implementation | Pinned and modified Music-Source-Separation-Training snapshot |
| Audio tools | Fixed FFmpeg/FFprobe build |
| Resampler | SoXR, precision 32 |
| Float dither | Disabled |
| Final integer dither | Triangular, once at final quantization |
| Processing concurrency | One song at a time |
| Default residual mode | 44.1 kHz compatibility mode |
| Experimental mode | Source sample rate residual |
| Distribution | One portable `soufmer.exe`; no MSI/NSIS installer or required sidecar |
| WebView | System Evergreen WebView2 on supported Windows 10/11 |
| Unit test scope | Core logic only |

---

## 4. Versioning policy

### Developer toolchain

Do not pin developer tools to exact patch releases.

- Rust: current stable supported by Tauri 2.
- pnpm: version 11 or newer.
- Node.js: 22.12 or newer.
- Use `pnpm-lock.yaml` and `Cargo.lock` to make builds reproducible.
- Do not add an exact-version `rust-toolchain.toml` for the MVP.
- Do not add an exact `packageManager` patch version unless a later CI problem requires it.

### End-user runtime

The end-user runtime should be tightly controlled.

- Python minor version: 3.11.
- Python patch: latest compatible patch selected by uv.
- Python packages: resolved and committed in `worker/uv.lock`.
- Torch and torchaudio: one tested CUDA wheel index and compatible versions selected explicitly
  in `worker/pyproject.toml`, then captured by `worker/uv.lock`.
- Music-Source-Separation-Training: pinned upstream commit or release snapshot.
- Model checkpoint: pinned Hugging Face revision and SHA-256.
- FFmpeg archive: pinned build URL, version, license type, and SHA-256.

`worker/pyproject.toml` plus `worker/uv.lock` are the only authority for the Python environment.
Do not repair an environment after sync with `uv pip install`, and do not leave packages present
only because a developer once ran `uv add`. Any required import must be declared, locked, and
reproducible from a clean environment.

---

## 5. Target repository layout

```text
.
├─ AGENTS.md
├─ IMPLEMENTATION_PLAN.md
├─ LICENSE
├─ THIRD_PARTY_NOTICES.md
├─ README.md
├─ package.json
├─ pnpm-lock.yaml
├─ tsconfig.json
├─ vite.config.ts
├─ components.json
├─ src/
│  ├─ app/
│  │  ├─ App.tsx
│  │  ├─ app-reducer.ts
│  │  └─ app-state.ts
│  ├─ components/
│  │  ├─ ui/
│  │  └─ feature/
│  │     ├─ main-form/
│  │     ├─ progress-dialog/
│  │     ├─ completion-dialog/
│  │     ├─ error-dialog/
│  │     └─ environment-status/
│  ├─ hooks/
│  │  ├─ use-backend-events.ts
│  │  └─ use-elapsed-time.ts
│  ├─ lib/
│  │  ├─ ipc.ts
│  │  ├─ schemas.ts
│  │  ├─ format.ts
│  │  └─ i18n.ts
│  ├─ locales/
│  │  ├─ en.json
│  │  └─ zh-CN.json
│  ├─ types/
│  │  ├─ backend.ts
│  │  └─ domain.ts
│  ├─ index.css
│  └─ main.tsx
├─ src-tauri/
│  ├─ Cargo.toml
│  ├─ Cargo.lock
│  ├─ build.rs
│  ├─ tauri.conf.json
│  ├─ capabilities/
│  │  └─ default.json
│  ├─ bootstrap/
│  │  ├─ bin/
│  │  │  └─ uv.exe
│  │  ├─ runtime-manifest.json
│  │  └─ licenses/
│  └─ src/
│     ├─ lib.rs
│     ├─ commands/
│     │  ├─ mod.rs
│     │  ├─ environment.rs
│     │  ├─ batch.rs
│     │  ├─ settings.rs
│     │  └─ diagnostics.rs
│     ├─ domain/
│     │  ├─ mod.rs
│     │  ├─ request.rs
│     │  ├─ progress.rs
│     │  ├─ result.rs
│     │  ├─ settings.rs
│     │  └─ error.rs
│     ├─ runtime/
│     │  ├─ mod.rs
│     │  ├─ embedded.rs
│     │  ├─ manifest.rs
│     │  ├─ layout.rs
│     │  ├─ status.rs
│     │  ├─ initializer.rs
│     │  ├─ downloader.rs
│     │  ├─ archive.rs
│     │  └─ self_test.rs
│     ├─ jobs/
│     │  ├─ mod.rs
│     │  ├─ manager.rs
│     │  ├─ planner.rs
│     │  ├─ runner.rs
│     │  └─ cleanup.rs
│     ├─ audio/
│     │  ├─ mod.rs
│     │  ├─ probe.rs
│     │  ├─ ffmpeg.rs
│     │  ├─ pipeline.rs
│     │  ├─ output.rs
│     │  └─ supported.rs
│     ├─ process/
│     │  ├─ mod.rs
│     │  ├─ command.rs
│     │  ├─ controller.rs
│     │  └─ windows_job.rs
│     ├─ worker/
│     │  ├─ mod.rs
│     │  ├─ request.rs
│     │  ├─ protocol.rs
│     │  └─ runner.rs
│     ├─ progress/
│     │  ├─ mod.rs
│     │  └─ aggregator.rs
│     ├─ storage/
│     │  ├─ mod.rs
│     │  ├─ settings_store.rs
│     │  └─ atomic_file.rs
│     └─ telemetry/
│        ├─ mod.rs
│        └─ logging.rs
├─ worker/
│  ├─ pyproject.toml
│  ├─ uv.lock
│  ├─ .python-version
│  ├─ README.md
│  ├─ src/
│  │  └─ accompaniment_worker/
│  │     ├─ __init__.py
│  │     ├─ __main__.py
│  │     ├─ cli.py
│  │     ├─ protocol.py
│  │     ├─ request.py
│  │     ├─ inference.py
│  │     ├─ errors.py
│  │     └─ self_test.py
│  ├─ vendor/
│  │  ├─ msst/
│  │  ├─ patches/
│  │  ├─ MSST_LICENSE
│  │  ├─ source-manifest.json
│  │  └─ UPSTREAM.md
│  ├─ configs/
│  │  └─ kimberley-melbandroformer.yaml
│  └─ tests/
│     ├─ test_protocol.py
│     ├─ test_request.py
│     ├─ test_config.py
│     └─ test_vendor_integrity.py
├─ tests/
│  └─ fixtures/
│     └─ generated-at-test-time.md
└─ docs/
   ├─ ARCHITECTURE.md
   ├─ RUNTIME_MANIFEST.md
   ├─ AUDIO_PIPELINE.md
   ├─ LICENSING.md
   └─ SMOKE_TEST.md
```

The exact module count may be reduced if a module would contain only trivial forwarding code. Do not preserve empty architectural layers merely to match this diagram.

---

## 6. Core domain model

Define the same concepts in Rust and TypeScript. Rust is authoritative and validates all incoming data.

### Input mode

```ts
type InputMode = "file" | "folder";
```

### Processing mode

```ts
type ProcessingMode = "compatibility44100" | "sourceSampleRate";
```

### Conflict policy

```ts
type ConflictPolicy = "skip" | "overwrite" | "autoNumber";
```

### Output format

Start with a small set:

```ts
type OutputFormat = "flac" | "wavFloat32";
```

Do not add MP3/AAC output in the first vertical slice. The source input may still be MP3 or AAC.

### Batch request

```ts
interface StartBatchRequest {
  inputMode: InputMode;
  inputPath: string;
  outputDirectory: string;
  processingMode: ProcessingMode;
  generateBothModes: boolean;
  recursive: boolean;
  preserveDirectoryStructure: boolean;
  conflictPolicy: ConflictPolicy;
  outputFormat: OutputFormat;
}
```

### Environment state

```ts
type EnvironmentStatus =
  | { type: "notInstalled"; estimatedDownloadBytes?: number; estimatedDiskBytes?: number }
  | { type: "installing"; runtimeVersion: string }
  | { type: "ready"; runtimeVersion: string; modelVersion: string; ffmpegVersion: string }
  | { type: "repairRequired"; reasonCode: string }
  | { type: "unsupported"; reasonCode: string };
```

### Task acknowledgement

```ts
interface TaskAcknowledgement {
  taskId: string;
  acceptedAt: string;
}
```

### Stable error

```ts
interface AppError {
  code: string;
  stage: string;
  messageKey: string;
  recoverable: boolean;
  diagnosticId: string;
  itemPath?: string;
}
```

---

## 7. Progress model

The same progress dialog supports initialization and audio processing.

### Generic progress value

```ts
interface ProgressValue {
  kind: "determinate" | "indeterminate";
  fraction?: number; // 0.0 to 1.0 when determinate
}
```

### Event envelope

```ts
interface BackendEvent<T> {
  schemaVersion: 1;
  taskId: string;
  sequence: number;
  timestamp: string;
  type: string;
  payload: T;
}
```

### Initialization progress

```ts
interface InitializationProgress {
  runtimeVersion: string;
  stepIndex: number;
  stepCount: number;
  stepId:
    | "checkingSystem"
    | "preparingTools"
    | "installingPython"
    | "syncingEnvironment"
    | "downloadingModel"
    | "selfTesting"
    | "activating";
  overall: ProgressValue;
  current: ProgressValue;
  bytesCompleted?: number;
  bytesTotal?: number;
  bytesPerSecond?: number;
  completedUnits?: number;
  totalUnits?: number;
  detail?: string;
}

interface InitializationActivity {
  stepId: InitializationProgress["stepId"];
  level: "status" | "download" | "install" | "warning";
  message: string;
  packageName?: string;
  completedUnits?: number;
  totalUnits?: number;
}
```

`InitializationActivity.message` is a short, sanitized, localized-or-localizable activity message,
not an arbitrary terminal transcript. The backend may derive it from trusted state transitions or
conservatively recognized uv output, but it must remove ANSI control sequences, command lines,
credentials, authorization headers, and unbounded paths. Store the full bounded stderr separately in
diagnostics.

### Batch progress

```ts
interface BatchProgress {
  itemIndex: number;
  itemCount: number;
  currentInputPath: string;
  currentDisplayName: string;
  stage:
    | "probing"
    | "preparingInput"
    | "separating"
    | "buildingCompatibilityOutput"
    | "buildingSourceRateOutput"
    | "validatingOutput"
    | "cleaningUp";
  overall: ProgressValue;
  current: ProgressValue;
  completedDurationSeconds: number;
  totalDurationSeconds: number;
  elapsedSeconds: number;
}
```

### Processing stage weights

Use provisional weights until measured profiling data is available:

| Stage | Weight |
|---|---:|
| Probe | 0.01 |
| Prepare model input | 0.04 |
| Model inference | 0.85 |
| Build output or outputs | 0.07 |
| Validate and publish | 0.02 |
| Cleanup | 0.01 |

When both outputs are enabled, divide the post-processing weight across both outputs.

### Batch overall progress

Use audio duration weighting:

```text
overall =
  (completed item durations + current item duration × current item fraction)
  / total item durations
```

If probing has not yet determined all durations, show overall progress as indeterminate while displaying `itemIndex / itemCount`. Probe all selected files before model processing so determinate duration-weighted progress can begin before the first inference.

### Initialization overall progress

Do not pretend that each step has equal duration.

Initial provisional weights:

| Step | Weight |
|---|---:|
| Check system | 0.02 |
| Validate/extract embedded bootstrap and prepare directories | 0.03 |
| Install Python | 0.10 |
| Sync Python environment | 0.45 |
| Download model | 0.35 |
| Self-test | 0.04 |
| Activate | 0.01 |

For download steps, map byte progress into the step's weight. For uv sync or other commands without
reliable total work, keep current progress indeterminate and only advance overall progress at safe
milestones. Never fabricate a percentage from elapsed time, parse a cosmetic spinner as progress, or
let either progress bar move backward or restart.

For long indeterminate work, continue to show the stable step number, elapsed time, a concise current
activity message, and a bounded activity feed. If reliable byte totals or completed/total unit counts
become available, the current-step bar may transition to determinate mode. The overall bar remains
stage-weighted and monotonic.

---

## 8. Runtime directory layout

### Single-executable bootstrap contract

The release artifact is the raw Tauri application executable produced with
`pnpm tauri build --no-bundle`. Tauri's normal `bundle.resources` mechanism copies additional
files to a `$RESOURCE` directory, so it must not be used for files required by the release. The
frontend assets are compiled into the Tauri binary, and every non-frontend bootstrap file is
packed into a deterministic archive by `src-tauri/build.rs` and compiled into the Rust binary with
`include_bytes!`.

The embedded bootstrap archive contains:

- The pinned Windows x64 `uv.exe`.
- `worker/pyproject.toml`, `worker/uv.lock`, and the worker package source.
- The minimal pinned MSST inference snapshot, the ordered local patch record, `UPSTREAM.md`, and
  the original MSST MIT license.
- The exact model configuration and its source record.
- The trusted runtime manifest and all notices that must be available before downloads complete.

The archive does not contain Python, Torch, torchaudio, FFmpeg, the model checkpoint, or WebView2.
Those large runtime components are either downloaded into the private runtime or, in WebView2's
case, supplied and updated by supported Windows.

`build.rs` must:

- Enumerate a fixed set of bootstrap roots, sort paths ordinally, normalize archive paths to `/`,
  and produce repeatable bytes for identical inputs.
- Emit `cargo:rerun-if-changed` for every bootstrap input.
- Create an entry manifest containing each relative path, byte length, and SHA-256, then expose the
  archive length, archive SHA-256, and bootstrap version as compile-time Rust constants.
  The entry manifest is covered by the outer archive digest and does not attempt to include its own
  recursively defined hash.
- Fail the release build on a symlink/reparse-point input, path escape, duplicate normalized path,
  missing license, placeholder manifest value, missing lockfile, or unrecorded MSST source file.
- Verify that `worker/uv.lock` contains no Git/VCS dependency, mutable branch reference, network
  source for MSST, or absolute developer-machine path.

At runtime, never extract beside the executable or into `%TEMP%`. Acquire one application-wide
Windows named mutex before mutating shared runtime state, verify the compiled archive digest, and
extract into `%LOCALAPPDATA%\soufmer\staging\bootstrap-<uuid>`. Reject absolute paths, `..`, drive
prefixes, alternate data stream names, links/reparse points, duplicates, and entries that exceed
declared count or size limits. Verify every extracted file against the entry manifest before
atomically moving the directory to `bootstrap\versions\<bootstrap-id>` and writing `READY`. An
existing matching bootstrap can be reused only after validation.

The executable may be copied, renamed, or replaced without moving the runtime. Multiple copies
share the same data root and initialization mutex. The executable never modifies itself and never
assumes its containing directory is writable.

The one-file contract still permits normal Windows system dependencies. Use the Evergreen
WebView2 Runtime maintained by Windows rather than embedding a fixed WebView2 directory or an
installer. Perform a native preflight before creating the Tauri window when practical. If WebView2
is absent, show a small native localized error with a trusted Microsoft recovery link and exit;
do not download or install a system-wide component silently. This is the only intentional runtime
prerequisite outside `%LOCALAPPDATA%\soufmer\`.

### Application data root

Resolve the Windows `FOLDERID_LocalAppData` known folder and append the literal `soufmer`
directory. The result must be exactly `%LOCALAPPDATA%\soufmer\`; do not use a Tauri
bundle-identifier-derived path, the current working directory, the executable directory, or the
user's global uv directories.

All application-managed writes—including bootstrap extraction, settings, logs, downloads, uv
cache, managed Python, environments, models, FFmpeg, job intermediates, and diagnostics—belong
under this root. User-selected source files and final output directories are outside this storage
rule by design. A failure to resolve or write this root is fatal and maps to a stable environment
error.

```text
%LOCALAPPDATA%\soufmer\
├─ bootstrap/
│  └─ versions/
│     └─ <bootstrap-id>/
│        ├─ uv.exe
│        ├─ worker/
│        ├─ runtime-manifest.json
│        ├─ licenses/
│        └─ READY
├─ state/
│  ├─ settings.json
│  ├─ current-bootstrap.json
│  ├─ current-runtime.json
│  └─ runtime-history.json
├─ runtime/
│  └─ versions/
│     └─ <runtime-id>/
│        ├─ worker/
│        ├─ python/
│        ├─ venv/
│        ├─ self-test.json
│        └─ READY
├─ tools/
│  └─ ffmpeg/
│     └─ <version>/
│        ├─ ffmpeg.exe
│        ├─ ffprobe.exe
│        └─ licenses/
├─ models/
│  └─ kimberley-melbandroformer/
│     └─ <revision>/MelBandRoformer.ckpt
├─ downloads/
│  ├─ *.part
│  └─ completed/
├─ staging/
├─ cache/
│  └─ uv/
├─ jobs/
│  └─ <task-id>/
│     └─ <item-id>/
├─ logs/
└─ diagnostics/
```

### Runtime activation

`current-runtime.json` should contain only validated data:

```json
{
  "schemaVersion": 1,
  "runtimeId": "runtime-1-cuda-<manifest-digest-prefix>",
  "activatedAt": "2026-07-30T00:00:00Z"
}
```

A runtime is valid only if:

- Its directory exists.
- `READY` exists.
- Its self-test record matches the runtime manifest digest and selected accelerator profile.
- Python, worker, model, FFmpeg, and FFprobe paths exist.

Create a candidate under its final unique `runtime/versions/<runtime-id>/` path and leave it
inactive until every check passes. Do not build a virtual environment under `staging/` and move it
later: venv metadata and generated entry points may contain absolute paths. Activation is the
atomic metadata switch in `current-runtime.json`, not relocation of the runtime directory.

---

## 9. Runtime manifest design

Place the trusted source manifest in `src-tauri/bootstrap/runtime-manifest.json` and include it in
the hash-verified embedded bootstrap archive.

Initial schema:

```json
{
  "schemaVersion": 1,
  "runtimeVersion": "1",
  "platform": "windows-x86_64",
  "python": {
    "version": "3.11",
    "implementation": "cpython"
  },
  "uv": {
    "version": "REPLACE_AT_RELEASE",
    "bootstrapRelativePath": "uv.exe",
    "sha256": "REPLACE_AT_RELEASE"
  },
  "worker": {
    "version": "1",
    "bootstrapRelativePath": "worker",
    "entryModule": "accompaniment_worker",
    "msstCommit": "REPLACE_AT_RELEASE",
    "configRelativePath": "configs/kimberley-melbandroformer.yaml",
    "configSha256": "REPLACE_AT_RELEASE"
  },
  "ffmpeg": {
    "version": "REPLACE_AT_RELEASE",
    "archiveUrl": "REPLACE_AT_RELEASE",
    "archiveSha256": "REPLACE_AT_RELEASE",
    "archiveType": "zip",
    "ffmpegRelativePath": "bin/ffmpeg.exe",
    "ffprobeRelativePath": "bin/ffprobe.exe",
    "licenseKind": "REPLACE_WITH_LGPL_OR_GPL"
  },
  "model": {
    "id": "KimberleyJSN/melbandroformer",
    "revision": "REPLACE_AT_RELEASE",
    "fileName": "MelBandRoformer.ckpt",
    "downloadUrl": "REPLACE_WITH_REVISION_SPECIFIC_HTTPS_URL",
    "sha256": "REPLACE_AT_RELEASE"
  },
  "profiles": {
    "cuda": {
      "uvExtra": "cuda",
      "torchBackend": "REPLACE_WITH_FIXED_BACKEND",
      "torchVersion": "REPLACE_AT_RELEASE",
      "torchaudioVersion": "REPLACE_AT_RELEASE",
      "estimatedDownloadBytes": 0,
      "estimatedInstalledBytes": 0,
      "minimumFreeDiskBytes": 0
    }
  }
}
```

### Manifest requirements

- Reject unknown schema versions.
- Reject non-HTTPS URLs in release manifests.
- Do not allow a bootstrap entry, runtime download, tool path, cache path, or job path to resolve
  outside `%LOCALAPPDATA%\soufmer\`.
- Keep development overrides behind a compile-time development flag or explicit environment variable.
- Never ship placeholder values in a release build.
- Reject zero release estimates and calculate free-space requirements from measured peak
  installation use, including downloads, uv cache, staging, the environment, and the model.
- Make every artifact URL immutable or revision-specific. A mutable `main` URL is not a release
  pin even when a SHA-256 is also checked.
- Include the selected runtime profile and a digest of this manifest in the runtime ID and
  self-test record so activation cannot accidentally combine components from different releases.

---

## 10. Tauri command API

Keep the frontend API narrow.

### `get_environment_status`

Request: none.
Response: `EnvironmentStatus`.

Responsibilities:

- Read the manifest.
- Inspect active runtime metadata.
- Return `ready`, `notInstalled`, `repairRequired`, or `unsupported`.
- Do not start installation.

### `initialize_environment`

Request: none.
Response: `TaskAcknowledgement` immediately after the backend accepts ownership.

Behavior:

1. Reject if another task or initializer is active.
2. Revalidate the embedded bootstrap descriptor and private data root.
3. Run the initialization or repair state machine.
4. Emit typed initialization progress and one terminal event.
5. Never accept a URL, command, package name, or destination path from the frontend.

### `get_app_settings`

Request: none.
Response: persisted settings with safe defaults.

### `save_app_settings`

Request: settings.
Response: saved settings.

Validate enum values and paths. Use atomic write.

### `validate_batch_request`

Request: `StartBatchRequest`.
Response:

```ts
interface ValidatedBatchSummary {
  inputCount: number;
  totalDurationSeconds?: number;
  estimatedOutputCount: number;
  warnings: string[];
}
```

Validation includes:

- Input exists.
- File input is a supported audio type.
- Folder input contains supported files.
- Output directory can be created or written.
- Input and output path relationship is safe.
- Multi-channel files are rejected or reported before processing.
- No planned final path is identical to a source path after canonical, case-insensitive
  comparison.

### `start_batch`

Request: `StartBatchRequest`.
Response: `TaskAcknowledgement` immediately after the backend accepts ownership.

Behavior:

1. Reject if another task is active.
2. Validate the request again.
3. Reject with `ENV_NOT_INITIALIZED` if the environment is not ready.
4. Start sequential batch processing.
5. Emit progress events.
6. Return the final result through the completion event.

### `cancel_active_task`

Request:

```ts
interface CancelTaskRequest {
  taskId: string;
}
```

Response: cancellation accepted or already terminal.

Behavior:

- Set cancellation token.
- Kill the current process tree.
- Clean current item temporary and partial files.
- Preserve completed outputs.
- Emit cancelled result.

### `get_diagnostic_report`

Request: diagnostic ID.
Response: user-copyable English diagnostic text.

The report may include paths, versions, stages, exit codes, and recent stderr. It must not include secrets.

### `reveal_output_directory`

Prefer the scoped Tauri opener plugin. If implemented in Rust, accept only a validated directory produced by the current or latest task.

---

## 11. Event API

Use a small number of stable event names.

```text
runtime://progress
runtime://activity
batch://progress
batch://item-completed
batch://completed
task://failed
task://cancelled
```

`runtime://activity` carries `InitializationActivity` entries for the bounded activity feed. It is
informational and must not be required to reconstruct task state; `runtime://progress` and terminal
events remain authoritative.

### Item completion payload

```ts
interface ItemCompletedPayload {
  itemIndex: number;
  inputPath: string;
  outputs: string[];
  durationSeconds: number;
  warnings: string[];
}
```

### Batch completion payload

```ts
interface BatchResult {
  taskId: string;
  outputDirectory: string;
  succeeded: number;
  failed: number;
  skipped: number;
  cancelled: boolean;
  items: BatchItemResult[];
}
```

### Event sequencing

- Increment `sequence` for every event in a task.
- Frontend ignores an event with a sequence number not greater than the last seen sequence for that task.
- Frontend ignores events from an old task after a new task becomes active.

---

## 12. Python worker protocol

Use a request JSON file rather than many command-line arguments.

### Invocation

Conceptual command:

```powershell
<venv-python.exe> -m accompaniment_worker separate --request <request.json>
```

The Rust process launcher passes arguments directly and does not invoke a shell.

### Windows path boundary

Rust filesystem canonicalization on Windows may produce verbatim paths beginning with `\\?\`.
Keep canonical or verbatim paths internally when they are useful for containment and identity checks,
but do not serialize them directly into the normal worker request or pass them to FFmpeg, FFprobe, uv,
or Python unless that exact downstream boundary has been proven to require and support them.

After canonical validation, derive an external-process path representation with these rules:

- `\\?\C:\path` becomes `C:\path`.
- `\\?\UNC\server\share\path` becomes `\\server\share\path`.
- Already-normal drive and UNC paths remain unchanged.
- Device namespaces such as `\\.\`, volume GUID paths, malformed prefixes, and paths that cannot be
  represented safely for the target process are rejected rather than rewritten heuristically.
- If a path requires verbatim syntax solely because of its length and downstream compatibility has
  not been demonstrated, fail before process launch with a stable localized `PATH_UNSUPPORTED` error
  instead of allowing a generic `INFERENCE_FAILED`.

Use separate variables or types for canonical internal paths, user-display paths, and process/worker
paths so a later refactor cannot accidentally reintroduce the prefix. Apply the conversion at the
external-process/request boundary, after security checks and before JSON serialization or argument
construction.

The worker validates the request again before importing the model:

- The input must probe as 44.1 kHz, stereo, Float32 PCM.
- Input, request, and vocals-output paths must remain within the assigned job directory after
  canonicalization.
- Checkpoint and configuration paths must match the active runtime's read-only paths.
- `device`, `batchSize`, and `overlap` must be selected from bounded values owned by the backend;
  arbitrary Python or Torch options are not accepted.

### Request schema

```json
{
  "schemaVersion": 1,
  "taskId": "uuid",
  "inputPath": "C:\\...\\model-input.wav",
  "outputVocalsPath": "C:\\...\\vocals.wav",
  "checkpointPath": "C:\\...\\MelBandRoformer.ckpt",
  "configPath": "C:\\...\\kimberley-melbandroformer.yaml",
  "device": "cuda:0",
  "batchSize": 1,
  "overlap": 2
}
```

### Standard output

Standard output is JSON Lines only.

Examples:

```json
{"schemaVersion":1,"type":"ready","taskId":"...","payload":{"device":"cuda:0"}}
{"schemaVersion":1,"type":"stage","taskId":"...","payload":{"stage":"loadingModel"}}
{"schemaVersion":1,"type":"stage","taskId":"...","payload":{"stage":"separating"}}
{"schemaVersion":1,"type":"progress","taskId":"...","payload":{"current":12,"total":48}}
{"schemaVersion":1,"type":"completed","taskId":"...","payload":{"outputPath":"C:\\...\\vocals.wav"}}
```

Error example:

```json
{"schemaVersion":1,"type":"error","taskId":"...","payload":{"code":"CUDA_OUT_OF_MEMORY","recoverable":true,"message":"CUDA out of memory"}}
```

### Standard error

- Python logging and tracebacks go to stderr.
- Rust captures stderr into the diagnostic log.
- stderr is not parsed as a user protocol.

### Exit codes

Suggested stable exit codes:

| Code | Meaning |
|---:|---|
| 0 | Success |
| 2 | Invalid request |
| 10 | Runtime import failure |
| 11 | Model load failure |
| 12 | CUDA unavailable |
| 13 | CUDA out of memory |
| 14 | Inference failure |
| 15 | Output write failure |
| 130 | Cancelled or terminated |

The JSON error code is authoritative when available.

Load the hash-verified checkpoint on CPU with `torch.load(..., weights_only=True,
map_location="cpu")` when the pinned checkpoint format supports it, then validate the state-dict
shape before loading it into the model. Any need for `weights_only=False` is a release-blocking
security exception that must be documented and justified; a PyTorch checkpoint is a pickle-capable
format and must not be treated like inert data.

---

## 13. Vendored MSST strategy

Do not run `git clone` during user initialization.

### Development workflow

1. Select a known-good Music-Source-Separation-Training commit.
2. Record the full 40-character commit, repository URL, retrieval date, copied relative paths, and
   pre-patch SHA-256 values in machine-readable `worker/vendor/source-manifest.json`; summarize the
   provenance for humans in `worker/vendor/UPSTREAM.md`. Never vendor from an unrecorded moving
   branch.
3. Start from the upstream MSST repository, not from an unrelated repackaging of the model.
4. Trace the import closure for the pinned `mel_band_roformer` inference path and copy only that
   closure into `worker/vendor/msst`. Configure the worker build backend to install it under a
   private worker-owned package namespace; do not add `worker/vendor` to `sys.path` at runtime.
   Do not copy training, dataset, GUI, web, multi-model, or folder-scanning code merely because it
   exists upstream.
5. Copy the exact upstream MIT license to `worker/vendor/MSST_LICENSE` and preserve copyright
   headers in copied files.
6. Record each local change as a small patch under `worker/vendor/patches/`. Keep the vendored
   working copy equivalent to the pinned snapshot plus the ordered patch set.
7. Add a repeatable developer-only vendor refresh/check script or documented command that verifies
   file hashes, applies patches, and reports drift. Git is allowed for this developer workflow but
   is never required on an end-user machine.
8. Re-run the import-closure test, lightweight worker tests, clean uv sync, and one real GPU smoke
   test after any upstream refresh.

Do not install the upstream MSST project or its broad `requirements.txt`/default dependency set
into the product environment. The upstream project includes training and GUI dependencies that
the one-model worker does not need. The worker owns a small, audited dependency list.

For the current Mel-Band RoFormer implementation, the expected direct runtime import set includes
Torch, NumPy, librosa, SoundFile, PyYAML/config loading, `ml-collections`, `einops`, `beartype`, and
`rotary-embedding-torch`; torchaudio is retained as a version-matched runtime dependency and
self-test target. This is a starting audit list, not permission to guess: confirm it against the
exact pinned MSST commit and declare every required package in `worker/pyproject.toml`.

The upstream `mel_band_roformer` extra may contain packages used by newer optional architecture
features, such as PoPE. Do not include such a package unless the pinned Kimberley configuration or
the vendored import path actually requires it.

### Required modifications

- Expose a library function for exactly one pre-normalized 44.1 kHz stereo Float32 input.
- Instantiate only `mel_band_roformer` from the pinned configuration.
- Reuse the pinned MSST chunking, overlap, windowing, and inference math.
- Write only one 44.1 kHz stereo Float32 vocals result into the assigned job directory.
- Remove or bypass folder enumeration, generic filename templates, instrumental generation,
  plotting, TTA, training, and unrelated model dispatch.
- Add a progress callback at the chunk/batch loop with stable `(completedUnits, totalUnits)`
  semantics and route it through the worker JSON Lines protocol.
- Disable `tqdm` and all ordinary upstream `print` calls on stdout. If unavoidable third-party
  output occurs, redirect it to stderr without swallowing diagnostics.
- Convert known CUDA OOM exceptions to `CUDA_OUT_OF_MEMORY`; map import, configuration, checkpoint,
  device, inference, and output failures to stable codes.
- Load the checkpoint strictly and fail on missing, unexpected, or shape-mismatched keys; do not
  reuse upstream's tolerant training checkpoint loader for product inference.
- Keep model imports lazy so request/protocol tests do not import Torch or initialize CUDA.
- Validate output sample rate, channels, length, finiteness, and Float32 subtype before emitting
  `completed`.

### Do not modify unnecessarily

- Model architecture definitions.
- Checkpoint tensor names.
- Kimberley configuration structure.
- Core inference mathematics.
- Chunk padding, overlap-add windows, mixed-precision behavior, or output scaling without a
  measured compatibility reason and a recorded patch.

### uv dependency and CUDA policy

The following manual repair sequence is explicitly prohibited for a managed project environment:

```powershell
uv pip install --reinstall torch torchaudio --torch-backend=auto
uv add einops beartype rotary-embedding-torch
```

It fails reproducibility for two separate reasons:

- `uv pip install` changes the environment without changing project metadata or `uv.lock`; a later
  project sync can replace that Torch build with the locked build.
- `uv add` is a development-time metadata edit, not an installation step to repeat after every
  sync. The resulting `pyproject.toml` and regenerated lockfile must be committed with the worker.

`--torch-backend=auto` and `[tool.uv.pip].torch-backend` apply to uv's pip interface, not to
`uv lock`, `uv sync`, or `uv run`. Auto-detect may be used once in a disposable developer
environment to learn which backend works, but the release must select one tested CUDA index and
lock it. This keeps first-run installation deterministic across machines. Supporting multiple
CUDA backends later requires separate pinned, tested runtime profiles; it must not be implemented
as an unrecorded post-sync mutation.

Use a project configuration equivalent to the following. Replace all placeholders during Phase 7
after testing a mutually compatible Torch/torchaudio/backend set on Windows and verifying that the
pinned versions support Python 3.11:

```toml
[project]
name = "accompaniment-worker"
version = "0.1.0"
requires-python = ">=3.11,<3.12"
dependencies = [
  "beartype",
  "einops",
  "librosa",
  "ml-collections",
  "numpy",
  "pyyaml",
  "rotary-embedding-torch",
  "soundfile",
]

[project.optional-dependencies]
cuda = [
  "torch==REPLACE_WITH_TESTED_VERSION",
  "torchaudio==REPLACE_WITH_THE_MATCHING_VERSION",
]

[tool.uv]
environments = [
  "sys_platform == 'win32' and platform_machine == 'AMD64'",
]
required-environments = [
  "sys_platform == 'win32' and platform_machine == 'AMD64'",
]

[tool.uv.sources]
torch = [{ index = "pytorch-cuda", extra = "cuda" }]
torchaudio = [{ index = "pytorch-cuda", extra = "cuda" }]

[[tool.uv.index]]
name = "pytorch-cuda"
url = "https://download.pytorch.org/whl/REPLACE_WITH_FIXED_BACKEND"
explicit = true
```

Add compatible version bounds for the non-Torch dependencies after the pinned MSST import audit,
then let `uv.lock` capture exact artifacts. Do not blindly copy upstream's broad dependency list.
The Windows environment restriction reflects the product scope, while `required-environments`
makes uv fail the lock if a wheel-only package such as Torch lacks a Windows x64 artifact.
If Torch or torchaudio also appears in base `project.dependencies`, uv can resolve the Windows
PyPI build when the CUDA extra is absent; keep them owned by the accelerator extra only.

The canonical commands are:

```powershell
# Development: update the lock only when dependency metadata intentionally changes.
uv lock --project worker

# Development: create the locked CUDA environment.
uv sync --project worker --locked --extra cuda

# GPU-aware development command. This remains synchronized and needs no --no-sync escape hatch.
uv run --project worker --locked --extra cuda python -m accompaniment_worker self-test
```

`uv run` uses an inexact environment sync by default, so a plain `uv run --project worker python`
after the CUDA extra has been synchronized should not remove the extra packages. Nevertheless,
commands that require Torch or the model must state `--extra cuda`; this also makes a clean
environment behave correctly. Lightweight protocol/configuration tests must not require that
extra. Never make `--no-sync` the normal development instruction because it hides drift between
the environment and the lockfile.

The packaged application never launches the worker through `uv run`. It uses uv only during
versioned runtime installation, then executes the validated
`<runtime>/venv/Scripts/python.exe -I -m accompaniment_worker ...` directly. Set `TEMP` and `TMP`
to the current job directory and route library caches such as `TORCH_HOME`, `HF_HOME`,
`XDG_CACHE_HOME`, and `NUMBA_CACHE_DIR` to versioned subdirectories under
`%LOCALAPPDATA%\soufmer\cache\`. Use `PYTHONNOUSERSITE=1`. Normal app startup and audio jobs
therefore cannot trigger a sync or write to user-global Python/cache locations.

---

## 14. Audio pipeline details

## 14.1 Source inspection

Run FFprobe and parse JSON.

Required fields:

- Selected audio stream index.
- Codec name.
- Sample rate.
- Channel count.
- Channel layout when available.
- Duration.
- Sample format.
- Bits per sample.
- Bits per raw sample.
- Container format.
- Tags needed for future metadata copying.

Reject:

- No audio stream.
- More than two channels in the MVP.
- Unsupported or unreadable input.
- Duration less than a small valid threshold.

Supported input extensions should be treated as a prefilter only. FFprobe is the authority.
Use one documented stream-selection rule (initially the first valid audio stream), record its
absolute FFprobe stream index, and reuse that exact index in every FFmpeg command for the item.

Initial extension set:

```text
.wav .flac .mp3 .m4a .aac .ogg .opus .aiff .aif .wma
```

If FFmpeg can decode an extension but it is not listed, defer support until manually tested.

## 14.2 Job directory

For each item:

```text
jobs/<task-id>/<item-id>/
├─ request.json
├─ source-info.json
├─ input/
│  └─ model-input.wav
├─ model/
│  └─ vocals.wav
├─ native/
│  └─ source-native.wav        # experimental mode only
├─ logs/
│  ├─ ffmpeg-prepare.log
│  ├─ worker-stderr.log
│  └─ ffmpeg-output.log
└─ state.json
```

Do not pre-create large temporary audio for every song. Create it only when processing that item.

## 14.3 Model input command

Construct an FFmpeg argument list equivalent to:

```text
-hide_banner
-nostdin
-y
-i <source>
-map 0:<selected-audio-stream-index>
-vn
-af aresample=resampler=soxr:osr=44100:precision=32
-ac 2
-c:a pcm_f32le
<model-input.wav>
```

Requirements:

- No shell quoting.
- SoXR precision 32.
- No dither.
- Output must probe as 44.1 kHz, two channels, Float32 PCM.

## 14.4 Compatibility residual

Inputs:

- `model-input.wav`
- `vocals.wav`

Use Float32 processing and subtract vocals with normalization disabled.

Conceptual `-filter_complex` argument value:

```text
[0:a:0][1:a:0]amix=inputs=2:duration=first:dropout_transition=0:weights=1 -1:normalize=0[out]
```

Rust passes that entire filter graph as one argument followed by `-map` and `[out]`. Do not add
shell quotes or a backslash before the space in `weights`; those are shell notation, not filter
syntax. Cover the exact argument vector with a focused test against the selected FFmpeg build.

Requirements:

- Use the exact existing model input file.
- Do not run a second 44.1 kHz conversion.
- Output sample rate is 44.1 kHz.
- Output duration follows the mixture input.
- Validate the output with FFprobe.

## 14.5 Experimental source-rate residual

1. Decode the selected source stream to `source-native.wav` as stereo Float32 at the source sample
   rate. Mono inputs are duplicated to stereo for the MVP so both residual inputs have the same
   channel layout.
2. Resample `vocals.wav` to the source sample rate using SoXR precision 32.
3. Subtract in Float32.
4. Use the source signal as the duration authority.
5. Encode to the selected output format.

Documented TODOs:

- Measure and compensate exact time offset.
- Measure resampling delay across supported sample-rate pairs.
- Add sample-count alignment and optional cross-correlation alignment.
- Add an explicit clipping strategy.
- Characterize high-frequency vocal residue above the model bandwidth.

The MVP implementation must log input and output durations and sample rates so later alignment work has diagnostic data.

## 14.6 Output encoding policy

### FLAC

Default output format.

- Compatibility mode sample rate: 44.1 kHz.
- Source-rate mode sample rate: source sample rate.
- Preserve common source bit depth of 16 or 24 where available.
- Use 24-bit when source depth is unknown, lossy, Float32, or unsupported by the chosen FLAC encoder settings.
- Apply triangular dither in one explicit final `aresample` sample-format conversion when
  converting Float32 to integer output. Pin and test the output sample format and raw-bit-depth
  arguments for 16-bit and 24-bit FLAC; do not assume the encoder adds the required dither.
- Probe `bits_per_raw_sample` as well as codec/sample format after encoding.

### WAV Float32

- Use `pcm_f32le`.
- Do not dither.
- Compatibility mode sample rate: 44.1 kHz.
- Source-rate mode sample rate: source sample rate.

All MVP outputs are stereo, including mono sources. Restoring mono output is deferred to
`AUDIO-005`; do not leave the experimental residual to implicit FFmpeg channel negotiation.

## 14.7 Output naming

Default filename template:

```text
<source stem> (Instrumental).<extension>
```

When both modes are generated:

```text
<source stem> (Instrumental - 44.1k).<extension>
<source stem> (Instrumental - Source SR).<extension>
```

If only source-rate mode is selected, keep the simple `(Instrumental)` name.

### Conflict policies

- `skip`: report skipped, do not process the model if all requested outputs already exist.
- `overwrite`: write to a new partial path and replace the existing final file only after validation.
- `autoNumber`: append ` (2)`, ` (3)`, and so on.

Never overwrite the input source path.

## 14.8 Partial-file publication

Write final encoding to a path in the final output directory such as:

```text
.<source-stem>.<task-id>.partial.<final-extension>
```

Keep the real media extension last and also pass the intended muxer/format explicitly to FFmpeg;
a path ending only in `.partial` is not sufficient for output-format inference.

After FFmpeg succeeds:

1. Probe the partial file.
2. Verify it has one valid audio stream.
3. Verify sample rate and channels.
4. Verify duration is within the current MVP tolerance.
5. Publish to the final path with a same-volume Windows-safe operation.

A failure must delete the partial file.

On Windows, a normal rename does not replace an existing destination. Implement overwrite
publication with a narrowly wrapped `ReplaceFileW` or equivalent tested same-volume replace
operation so the old validated output survives until the new partial has passed validation.
`skip` and `autoNumber` must still re-check the destination immediately before publication to
handle a file created after preflight.

---

## 15. Input enumeration and output planning

### File input

- One item.
- Output directory is always selected separately.
- Never ask for a full output filename in the main form.

### Folder input

- Enumerate supported extensions.
- Optional recursion.
- Sort deterministically by normalized relative path.
- Canonicalize the input root and output directory for containment checks using Windows
  case-insensitive semantics.
- If the output directory is inside the input tree, exclude it.
- Do not follow directory symlinks or junctions during MVP enumeration.
- Take a complete input snapshot before processing begins.
- Optional preservation of relative directory structure.

### Preflight

Before starting model inference:

- Probe every candidate file.
- Reject unsupported channel counts.
- Calculate durations.
- Plan output paths and conflict results.
- Report warnings.
- Calculate total duration for progress weighting.

A bad file should normally become an item failure, not abort the entire batch, unless the request itself is invalid or no valid items remain.

---

## 16. Process management and cancellation

### Process controller abstraction

All external processes should use one backend abstraction that supports:

- Executable path.
- Argument vector.
- Environment variables.
- Working directory.
- Explicit conversion from validated canonical paths to target-compatible process paths.
- stdout mode: JSON Lines, machine progress, or log only.
- stderr capture.
- Cancellation.
- Exit code.
- Windows hidden-window flags.
- Windows process-tree ownership.

The command builder must not receive a canonical `\\?\` path accidentally through a generic
`PathBuf` conversion. Path conversion failures must be reported before child creation with the
original path retained only in bounded diagnostics.

### Windows Job Object

Attach uv, Python, FFmpeg, and FFprobe child processes to a Job Object configured so closing the job kills all descendants.

Avoid the race in which a child launches descendants before it is assigned to the Job Object.
Create controlled processes suspended with no console window, assign them to the task's Job
Object, and only then resume the primary thread (or use an equivalently race-free Windows API
sequence). Keep raw Windows handles in a small platform module and test cleanup behavior.

Required behavior:

- No visible console window.
- Cancelling a task kills the current process tree.
- Closing the application during an active task kills descendants.
- A worker-spawned subprocess cannot remain orphaned.

### Cancellation checkpoints

Check cancellation:

- Before each initialization step.
- During downloads.
- Before and after each child process.
- Before publishing output.
- Between batch items.

### Cancel result

- Current partial output: delete.
- Current job directory: delete or retain only small diagnostics.
- Completed outputs: preserve.
- Remaining items: mark unprocessed.
- Emit `task://cancelled` once.

---

## 17. Environment initialization sequence

On the first application launch, render the normal Tauri UI, attach progress listeners, and enter
the initialization flow before processing is enabled. Because the download is large, show the
estimated download and installed sizes and require one explicit confirmation before network work
begins. A cancelled or failed first run remains safely retryable. On later launches, validate the
active bootstrap/runtime markers without running `uv sync`; reinitialize only when the embedded
manifest requires a new or repaired runtime. `start_batch` must reject an unready environment
rather than starting an unobserved installation.

### Step 1 — Check system

- Confirm Windows x64.
- Resolve the exact `%LOCALAPPDATA%\soufmer\` root and confirm it and the selected output path are
  writable.
- Check free disk space against manifest minimum.
- Check the embedded bootstrap descriptor and WebView2 prerequisite.
- Detect NVIDIA availability at a basic system level if practical.

Do not fail solely because `nvidia-smi` is absent. The final Python self-test is authoritative for CUDA.

### Step 2 — Activate embedded bootstrap and prepare tools

- Acquire the application initialization mutex.
- Validate and extract the embedded bootstrap archive using the rules in Section 8, or validate and
  reuse the matching active bootstrap version.
- Resolve `uv.exe`, the runtime manifest, worker project, model configuration, and license notices
  only from that activated bootstrap version.

- Create a unique candidate directly at its final versioned runtime path; it is not valid without
  `READY` and is not active without the metadata switch.
- Copy the extracted, hash-verified worker project into the candidate runtime.
- Use `downloads/` and `staging/` only for resumable artifacts and archive extraction.
- Download FFmpeg archive if the fixed version is not installed.
- Resume interrupted download when server behavior permits.
- Verify SHA-256.
- Extract into staging.
- Locate and validate `ffmpeg.exe` and `ffprobe.exe`.

### Step 3 — Install Python

Set private uv environment variables, for example:

```text
TEMP=%LOCALAPPDATA%\soufmer\staging\tmp\<operation-id>
TMP=%LOCALAPPDATA%\soufmer\staging\tmp\<operation-id>
UV_CACHE_DIR=%LOCALAPPDATA%\soufmer\cache\uv
UV_PYTHON_INSTALL_DIR=<candidate runtime>/python
UV_PYTHON_BIN_DIR=<candidate runtime>/python-bin
UV_PROJECT_ENVIRONMENT=<candidate runtime>/venv
UV_PYTHON_PREFERENCE=only-managed
UV_PYTHON_INSTALL_BIN=0
UV_PYTHON_INSTALL_REGISTRY=0
UV_NO_MODIFY_PATH=1
UV_NO_ENV_FILE=1
```

Run a command equivalent to:

```powershell
uv python install 3.11
```

Use the pinned embedded uv build, request managed Python only, and do not create global executable
aliases or Windows registry entries. Do not modify PATH. Record the resolved CPython patch/build
in the self-test record. Create the operation temp directory beneath the private root before
launch and clean it afterward. Never pass `--no-cache`, because uv would otherwise fall back to a
temporary cache; keep `UV_CACHE_DIR` on the same volume as the environment.

### Step 4 — Sync environment

Run from the worker project directory:

```powershell
uv sync --locked --no-dev --extra cuda --no-editable --managed-python --no-python-downloads
```

The working directory is the copied worker project and `UV_PROJECT_ENVIRONMENT` points to the
staged runtime venv. The CUDA extra and its fixed index come from `worker/pyproject.toml`; the
command performs no post-sync `uv pip install` or `uv add`. `--no-editable` ensures the activated
environment does not depend on the retained project source path through an editable link, so the
worker build configuration must include the vendored MSST package and configuration resources.

Capture stdout and stderr incrementally with `--color never` or equivalent ANSI-free output.
Do not depend on uv's terminal spinner or progress-bar rendering as a machine protocol. Emit
sanitized `runtime://activity` messages for recognized phases such as resolving, downloading,
preparing, installing, and auditing packages. Keep the current-step bar indeterminate unless uv or a
separate trusted measurement provides reliable byte or unit totals. Preserve the complete bounded
command output in diagnostics rather than exposing raw terminal output in the normal UI.

### Step 5 — Download model

- Download fixed checkpoint URL.
- Resume if supported.
- Verify SHA-256.
- Move into the versioned model directory.

### Step 6 — Self-test

Run a worker self-test that verifies:

- Python starts.
- Required imports succeed.
- Torch and torchaudio versions are reported.
- The reported Torch build suffix/backend and torchaudio version match the selected manifest
  profile and lockfile.
- CUDA is available.
- A tiny CUDA tensor operation succeeds on the selected device.
- Model configuration can be loaded.
- Checkpoint can be opened or minimally validated.
- FFmpeg and FFprobe execute.

Do not load and run a full song during normal initialization unless checkpoint validation cannot otherwise catch common packaging errors.

### Step 7 — Activate

- Write self-test result.
- Write `READY` marker.
- Atomically update `current-runtime.json`.
- Delete download/extraction staging leftovers.
- Continue directly into the user's batch.

### Failure behavior

- Keep existing active runtime unchanged.
- Keep a resumable download partial when safe.
- Remove invalid extraction directories and the inactive candidate runtime after preserving only
  bounded diagnostics.
- Emit a stable error code and diagnostic ID.
- Offer retry from the UI.

---

## 18. Downloader requirements

Implement a focused downloader rather than invoking PowerShell.

Required features:

- HTTPS.
- Destination `.part` file.
- Content length when available.
- Byte progress events.
- Cancellation.
- Bounded retries for transient network failures.
- Resume with HTTP Range when both local state and server response permit it.
- Store expected URL plus ETag/Last-Modified metadata beside a partial and require a valid `206`
  response with matching `Content-Range` before appending; otherwise restart the artifact.
- SHA-256 verification after complete download.
- Atomic move into completed location.
- Clear distinction between network failure and hash mismatch.

Do not implement a general download manager UI.

Initial retry policy:

- Maximum three attempts.
- Exponential delay with a small cap.
- Do not retry hash mismatch without deleting the bad completed file.
- Do not retry user cancellation.

---

## 19. Frontend UX specification

## 19.1 Window

Initial target:

- One main Tauri window.
- Start near 760 × 720 and set a measured minimum size no smaller than approximately 720 × 660;
  increase either value if the final collapsed layout clips at supported display scaling.
- Use a full-height flex/grid shell with `html`, `body`, and the application root constrained to the
  viewport and with page-level overflow hidden.
- Keep the environment status and primary action in a persistent bottom action region so they are
  visible without scrolling.
- Keep advanced options collapsed by default and reduce excess vertical spacing before increasing
  density elsewhere.
- At 100%, 125%, and 150% Windows scaling, the normal collapsed main screen must have no page-level
  scrollbar and no clipped controls. Test 175% and 200% as accessibility cases; if the screen cannot
  fit inside the monitor work area, use a bounded internal `ScrollArea` for the optional content
  region rather than hiding content or restoring a page scrollbar.
- No separate progress window.
- Use an in-window dialog or overlay.

### Final color system

Use a restrained Final Cut-inspired magenta family rather than the existing blue accent:

- Display accent: `#FF8AD8`, matching HSB 320°, 46%, 100%. Use it for progress fills, selected
  decoration, small icons, and other non-text emphasis.
- Primary filled controls: `#C12B8F` with white text; hover/pressed may use `#A61E73`.
- Strong border or focus accent on light surfaces: approximately `#E85BC0` or a darker token that
  maintains at least 3:1 non-text contrast.
- Soft selected/background tint: approximately `#FFF0FA` with dark foreground text.
- Destructive, warning, success, and neutral colors keep their semantic meanings and must not be
  recolored pink.

Implement the palette through shared shadcn/Tailwind theme tokens, not repeated component-local
classes. Do not place white text on `#FF8AD8`; the bright display accent is too light for normal white
text. Keep visible keyboard focus and verify text, component, and state contrast against WCAG 2.2.

## 19.2 Main form

Suggested layout:

```text
Application title and concise description

Input type segmented control
[Single file] [Folder]

Input path
[path field] [Browse]

Output directory
[path field] [Browse]

Processing mode radio cards
[Compatibility mode — recommended]
[Source sample rate — experimental]

Advanced options collapsible

Environment status row

[Start extraction]
```

### Processing mode Chinese copy

Use translations approximately equivalent to:

- Compatibility title: `兼容模式（推荐）`
- Compatibility description: `统一以 44.1 kHz 处理，残差匹配更稳定。`
- Source-rate title: `保留采样率（实验性）`
- Source-rate description: `输出采样率尽量与原文件一致，部分文件可能存在对齐误差。`

Keep the English translation complete even if it is not exposed prominently in the first release.

## 19.3 Advanced options

Use shadcn `Collapsible` or `Accordion`.

Fields:

- Include subfolders — folder input only.
- Preserve directory structure — folder input only.
- Output format — FLAC default, WAV Float32 alternative.
- Existing file policy — skip default.
- Generate both modes — off by default.

Hide irrelevant options rather than leaving them disabled without explanation.

## 19.4 Environment status

Examples:

```text
AI runtime: Not installed
First use requires approximately <manifest estimate> of downloads.
```

```text
AI runtime: Ready
Runtime 1 · CUDA · Model installed
```

When the first-launch status is `notInstalled` or `repairRequired`, open the initialization
confirmation after the main window and event listeners are ready. Disable `Start extraction` until
initialization succeeds. If the user cancels, retain an explicit `Initialize runtime` action in the
environment-status surface; do not defer setup invisibly to `start_batch`.

Do not hard-code the estimate in a translation. Format backend-provided byte values from the
selected release manifest so Torch/backend changes cannot leave the UI estimate stale.

## 19.5 Initialization confirmation

Show:

- Estimated download size.
- Estimated disk requirement.
- Note that later processing can run offline.
- Cancel and install actions.

After user confirmation, initialize and continue the already submitted batch automatically.

## 19.6 Progress dialog

Use shadcn `Dialog`, `Progress`, `Button`, `Badge`, `Separator`, and `ScrollArea`; use
`Collapsible` only for secondary details.

Display:

- Title: initialization or extraction.
- Overall progress label and monotonic bar.
- Count: step `m / n` or song `m / n`.
- Current step or filename.
- Current progress bar.
- Stage text and one concise current-activity line.
- Download bytes and speed when relevant.
- Elapsed time.
- During initialization, a bounded activity area showing the most recent sanitized messages, with
  automatic scrolling while the user has not manually scrolled upward.
- Cancel action.

For initialization step 4, keep the activity area visible rather than presenting only a bouncing bar.
Use clear copy such as “正在下载并安装大型 CUDA 组件，可能需要数分钟” after a short period without
a more specific update. Do not expose the executable command line, raw traceback, ANSI control
sequences, credentials, or an unbounded terminal transcript.

Do not show a fake percentage or unstable time-remaining estimate for indeterminate work. Prefer an
indeterminate current-step bar plus explicit activity and elapsed time. Use determinate progress only
for measured bytes or reliable completed/total work units.

## 19.7 Cancellation confirmation

Message meaning:

```text
Cancel the current batch?
The unfinished output for the current song will be removed. Completed songs will be kept.
```

Buttons:

- Continue processing.
- Cancel task.

After cancellation is accepted, disable repeated cancel clicks and display a `Cancelling…` state until the backend confirms termination.

## 19.8 Completion dialog

Display:

- Success count.
- Failure count.
- Skipped count.
- Output directory.
- Open output directory.
- View failed items when present.
- Done.

A partial-success batch must not be presented as a total failure.

## 19.9 Error dialog

Display:

- Localized title and description.
- Recovery action when available.
- Diagnostic code.
- Copy diagnostic report.
- Close or retry.

Raw stderr is not displayed unless a future developer mode is explicitly added.

---

## 20. Frontend component plan

Use the following shadcn components only when needed:

- `button`
- `input`
- `label`
- `card`
- `radio-group`
- `toggle-group` or `tabs` for input mode
- `select`
- `checkbox`
- `collapsible`
- `dialog`
- `alert-dialog`
- `progress`
- `badge`
- `separator`
- `tooltip`
- `scroll-area`
- `sonner` for small non-blocking notices

Feature components:

```text
MainForm
├─ InputModeSelector
├─ PathSelector
├─ ProcessingModeCards
├─ AdvancedOptions
└─ StartAction

EnvironmentStatus
TaskProgressDialog
CompletionDialog
ErrorDialog
FailedItemList
```

Do not install all shadcn components preemptively.

---

## 21. Frontend state machine

Implement in `src/app/app-reducer.ts`.

### States

```ts
type AppState =
  | { type: "booting" }
  | { type: "idle"; environment: EnvironmentStatus; settings: AppSettings }
  | { type: "validating"; request: StartBatchRequest }
  | { type: "awaitingInitializationConsent"; request: StartBatchRequest; environment: EnvironmentStatus }
  | { type: "initializing"; taskId: string; progress: InitializationProgress }
  | { type: "processing"; taskId: string; progress: BatchProgress }
  | { type: "cancelling"; taskId: string; lastProgress?: BatchProgress | InitializationProgress }
  | { type: "completed"; result: BatchResult }
  | { type: "failed"; error: AppError; previousEnvironment?: EnvironmentStatus };
```

### Important transitions

```text
booting -> idle
idle -> validating
validating -> awaitingInitializationConsent
validating -> processing
awaitingInitializationConsent -> initializing
initializing -> processing
processing -> completed
initializing|processing -> cancelling
cancelling -> completed(cancelled)
any active state -> failed
completed|failed -> idle
```

Invalid transitions should be ignored or logged in development, not allowed to corrupt state.

---

## 22. Settings persistence

Initial settings:

```ts
interface AppSettings {
  schemaVersion: 1;
  locale: "zh-CN" | "en";
  lastInputMode: InputMode;
  lastOutputDirectory?: string;
  processingMode: ProcessingMode;
  recursive: boolean;
  preserveDirectoryStructure: boolean;
  conflictPolicy: ConflictPolicy;
  outputFormat: OutputFormat;
  generateBothModes: boolean;
}
```

Defaults:

- Locale: `zh-CN`.
- Input mode: file.
- Processing mode: compatibility.
- Recursive: true.
- Preserve directory structure: true.
- Conflict policy: skip.
- Output format: FLAC.
- Generate both: false.

Use an atomic JSON write. On invalid settings, preserve the bad file for diagnostics and load defaults.

---

## 23. Logging and diagnostics

Use Rust `tracing`.

### Log format

Prefer one rolling text or JSON-lines log per day or per application session. Keep implementation simple.

Include fields:

- Timestamp.
- Level.
- Task ID.
- Item ID.
- Stage.
- Component.
- Error code.
- Process exit code.

### Diagnostic report

Build a text report containing:

- Application version.
- Windows version and architecture.
- Runtime version.
- Python version.
- Torch and torchaudio versions when known.
- CUDA availability.
- Selected CUDA backend and MSST commit.
- FFmpeg version.
- Model revision.
- Task and item identifiers.
- Input probe summary.
- Failed stage.
- Stable error code.
- Child process exit code.
- Last bounded stderr lines.

Do not include authorization headers or download credentials.

---

# Implementation phases

## Phase 0 — Repository bootstrap and decisions

### Tasks

- [x] Create or confirm the repository root.
- [x] Add `AGENTS.md` and the committed `IMPLEMENTATION_PLAN.md`.
- [x] Add an MIT `LICENSE` for original project code.
- [x] Add an initial `THIRD_PARTY_NOTICES.md` with exact records for MSST, model, uv, FFmpeg,
  Tauri, and major runtime components.
- [x] Confirm a working application name and reverse-domain identifier in one central location.
- [x] Confirm Node.js 22.12 or newer is available.
- [x] Confirm Windows Tauri prerequisites are installed: Microsoft C++ build tools and WebView2 development/runtime requirements.
- [x] Record actual installed versions in a local development note, not as hard repository pins.

### Suggested preflight commands

```powershell
rustc --version
cargo --version
node --version
pnpm --version
```

### Acceptance criteria

- Repository has licensing and agent instructions.
- Developer machine satisfies current Tauri prerequisites.
- No exact Rust or pnpm patch pin has been introduced.

---

## Phase 1 — Scaffold Tauri, React, TypeScript, Vite, Tailwind, and shadcn

### Tasks

- [x] Scaffold a Tauri 2 project using the React + TypeScript + pnpm template.
- [x] Confirm `pnpm tauri dev` opens the application.
- [x] Configure Vite according to the current Tauri Vite guide.
- [x] Add Tailwind CSS using the current Vite plugin integration.
- [x] Initialize shadcn/ui for the existing Vite project.
- [x] Configure the `@/` import alias consistently in TypeScript and Vite.
- [x] Add only the initial shadcn components needed for the static screen.
- [x] Add Lucide React.
- [x] Add ESLint and TypeScript checking if the scaffold does not already include suitable scripts.
- [x] Add `engines` with broad minimums, not exact versions.
- [x] Commit `pnpm-lock.yaml` and `src-tauri/Cargo.lock`.

### Initial scripts

Target scripts:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "typecheck": "tsc -b --pretty false",
    "lint": "eslint .",
    "test": "vitest run",
    "tauri": "tauri"
  }
}
```

Adjust to the scaffold's TypeScript project layout rather than forcing this exact command if it uses multiple tsconfig files.

### Acceptance criteria

- `pnpm dev` works.
- `pnpm tauri dev` works.
- Tailwind classes render.
- A shadcn button renders.
- Release frontend build succeeds.

---

## Phase 2 — Localization and static GUI shell

### Tasks

- [x] Install and configure `i18next` and `react-i18next`.
- [x] Add structurally matching `en.json` and `zh-CN.json`.
- [x] Make `zh-CN` the default locale and English the fallback.
- [x] Implement the main form with mocked local state.
- [x] Implement file/folder input selector UI.
- [x] Implement input and output path fields with disabled manual editing or controlled editing, depending on the chosen UX.
- [x] Implement processing mode radio cards.
- [x] Implement advanced options collapsible.
- [x] Implement environment status component using mocked status.
- [x] Implement progress dialog using mocked initialization and processing states.
- [x] Implement completion dialog.
- [x] Implement error dialog.
- [ ] Complete final spacing, typography, focus states, disabled states, high-DPI behavior,
  no-page-scroll layout, and the magenta theme in Phase 16A.
- [x] Ensure no Chinese text is embedded directly in JSX.

### Visual direction

- Neutral, restrained desktop utility aesthetic.
- One clear Final Cut-inspired magenta accent family; do not retain the scaffold's blue primary
  treatment.
- Avoid dashboard-like density and avoid using pink as a large page background.
- Use large click targets for path selection and the primary action.
- Avoid excessive animation.
- Use motion only for dialog transitions and indeterminate progress.

### Acceptance criteria

- Every planned MVP surface can be reached using mock state controls in development.
- Chinese UI is complete.
- English fallback is complete.
- Keyboard focus is visible.
- No router or global state dependency has been added.

---

## Phase 3 — Tauri plugins, backend skeleton, and fake progress task

### Tasks

- [x] Add the Tauri dialog plugin for file and folder selection.
- [x] Add the Tauri opener plugin for revealing output files/directories with narrow permissions.
- [x] Configure Tauri capabilities with the smallest required permissions.
- [x] Do not add broad shell permissions.
- [x] Create Rust domain types for settings, requests, progress, results, and errors.
- [x] Create the narrow command API skeleton.
- [x] Supersede the planned fake task with the completed production initialization and batch task
  implementations.
- [x] Implement frontend event subscriptions with cleanup.
- [x] Connect the reducer to actual Tauri commands and typed production backend events.
- [x] Handle stale event sequence numbers and task IDs.
- [x] Add one reducer lifecycle test.

### Fake task behavior

The fake task should:

- Emit initialization steps.
- Transition to processing automatically.
- Emit three fake songs with current and overall progress.
- Support cancellation.
- Emit completion summary.

This phase proves the complete GUI lifecycle before downloads and model work begin.

### Acceptance criteria

- The real main form starts a fake backend task.
- Both progress bars update.
- Initialization flows directly into processing.
- Cancellation reaches a terminal cancelled result.
- Completion and error dialogs receive typed backend data.

---

## Phase 4 — Private data root, settings, manifests, and environment status

### Tasks

- [x] Resolve `FOLDERID_LocalAppData` and enforce the exact `%LOCALAPPDATA%\soufmer\` root.
- [x] Prove with a focused test that the executable directory and current working directory do not
  influence the data root.
- [x] Implement the runtime directory layout.
- [x] Implement the application-wide initialization mutex and active-bootstrap metadata.
- [x] Implement atomic JSON writes.
- [x] Implement settings load/save with defaults and schema version.
- [x] Implement runtime manifest parsing and validation.
- [x] Add manifest documentation.
- [x] Implement active runtime state reading.
- [x] Implement `get_environment_status`.
- [x] Replace mocked environment status in the UI.
- [x] Add focused tests for manifest parsing and invalid hash formatting.

### Acceptance criteria

- A clean machine state reports `notInstalled`.
- A valid synthetic runtime marker reports `ready`.
- All synthetic application-managed paths remain beneath `%LOCALAPPDATA%\soufmer\`.
- Corrupt settings fall back to defaults without crashing.
- Unknown manifest schema is rejected with a stable error.

---

## Phase 5 — Process abstraction and Windows cancellation foundation

### Tasks

- [x] Implement a child process command builder using executable path and argument vector.
- [x] Hide child console windows on Windows.
- [x] Capture stdout and stderr separately.
- [x] Implement line-oriented stdout handling.
- [x] Implement bounded stderr buffering plus file logging.
- [x] Implement Windows Job Object ownership.
- [x] Implement process-tree termination.
- [x] Connect cancellation tokens to active processes.
- [x] Add a small test helper executable or use a safe system command in a Windows-only integration test to verify cancellation.

### Acceptance criteria

- A long-running test child process can be cancelled.
- Child descendants do not remain after cancellation.
- No console window flashes in the manual smoke test.
- User paths are never placed in shell command strings.

---

## Phase 6 — Downloader, archive extraction, and FFmpeg installation

### Tasks

- [x] Implement download staging and `.part` files.
- [x] Emit byte progress.
- [x] Implement cancellation.
- [x] Implement bounded retries.
- [x] Implement optional HTTP Range resume.
- [x] Implement SHA-256 verification.
- [x] Implement ZIP extraction with path traversal protection.
- [x] Install the fixed FFmpeg archive into a versioned directory.
- [x] Validate `ffmpeg -version` and `ffprobe -version`.
- [x] Record the FFmpeg build version and license files.
- [x] Add a small local HTTP test fixture or mocked downloader test only for core resume/hash behavior.

### Acceptance criteria

- A valid test archive downloads, verifies, extracts, and activates.
- A hash mismatch fails with `ENV_HASH_MISMATCH` and does not activate files.
- Cancellation leaves a safe resumable or deletable partial.
- FFmpeg and FFprobe paths are found through the manifest, not PATH.

---

## Phase 7 — Python worker project and uv environment

### Tasks

- [x] Create `worker/pyproject.toml`.
- [x] Set `.python-version` to `3.11`, not an exact patch.
- [x] Select a known-good exact MSST commit for the dependency/import audit and record the full SHA.
- [x] Audit the pinned MSST Mel-Band RoFormer import closure and declare all worker runtime
  dependencies, including `einops`, `beartype`, and `rotary-embedding-torch`.
- [x] Define version-matched Torch and torchaudio in a `cuda` extra mapped to one fixed, explicit
  PyTorch CUDA index through `tool.uv.sources`.
- [x] Verify from a clean environment that no `uv pip install --reinstall`, `uv add`, or
  `--no-sync` repair step is needed.
- [x] Generate and commit `uv.lock`.
- [x] Add a build backend and verify a non-editable install contains the worker package.
- [x] Add the worker package and CLI.
- [x] Add request validation.
- [x] Add JSON Lines protocol utilities.
- [x] Add worker self-test.
- [x] Add the worker project and vendored MSST tree to the deterministic embedded bootstrap inputs.
- [x] Add a release check that rejects Git/VCS MSST dependencies and absolute local paths in
  `uv.lock`.
- [x] Implement private uv environment variables.
- [x] Implement `uv python install 3.11`.
- [x] Implement the production sync command from Section 17 with `--locked`, `--no-dev`,
  `--extra cuda`, `--no-editable`, `--managed-python`, and `--no-python-downloads`.
- [x] Add core Python protocol tests only.

### Notes

- Development may use the local worker directory directly.
- Production copies the extracted, hash-verified embedded worker snapshot into the runtime version
  directory before sync.
- Do not use editable installs in production.

### Acceptance criteria

- A clean `%LOCALAPPDATA%\soufmer\` tree can install Python and sync the worker environment.
- A second locked sync is a no-op, and `uv run --project worker --locked --extra cuda ...` does not
  change the selected Torch build.
- A later startup with a ready runtime does not re-run sync.
- Worker self-test reports structured JSON.
- Global Python and PATH remain unchanged.

---

## Phase 8 — Pin and adapt Music-Source-Separation-Training

### Tasks

- [x] Retrieve the exact upstream commit selected in Phase 7 and verify its SHA before copying.
- [x] Record the exact copied-file inventory and source hashes.
- [x] Store that inventory in `worker/vendor/source-manifest.json` and verify it in the vendor
  refresh/check script.
- [x] Copy only the inference import closure into the packaged vendor namespace.
- [x] Preserve the exact upstream MIT license and copyright headers.
- [x] Add `worker/vendor/UPSTREAM.md` with repository URL, full commit, copy date, file inventory,
  dependency audit, and ordered local patches.
- [x] Add the exact Kimberley configuration from the same pinned MSST snapshot and record its hash.
- [x] Verify the non-editable worker install contains the vendored MSST package and required
  configuration resources.
- [x] Re-run the clean dependency audit and regenerate `uv.lock` if the copied import closure
  differs from the Phase 7 audit.
- [x] Add a repeatable vendor drift/patch check.
- [x] Implement single-file inference wrapper.
- [x] Remove or bypass folder scanning.
- [x] Disable uncontrolled tqdm output on stdout.
- [x] Emit chunk or batch progress through the worker protocol.
- [x] Map CUDA OOM and common import/checkpoint failures to stable codes.
- [x] Load the hash-verified checkpoint with `weights_only=True` and validate its state dict.
- [x] Confirm output vocals file is 44.1 kHz stereo Float32.
- [ ] Run one manual GPU inference against a short, legally usable test clip.

### Acceptance criteria

- Worker loads the Kimberley checkpoint.
- The vendored tree and patch records reproduce the audited pinned MSST source.
- Worker processes one controlled WAV.
- Worker stdout contains valid JSON Lines only.
- Worker produces a valid vocals WAV.
- Model inference progress is determinate when practical; otherwise stage remains explicitly indeterminate.

---

## Phase 9 — FFprobe and model-input audio pipeline

### Tasks

- [x] Implement FFprobe JSON parsing.
- [x] Implement supported input detection.
- [x] Reject more than two channels.
- [x] Implement model-input conversion with SoXR precision 32.
- [x] Validate generated model-input properties.
- [x] Implement job directory creation and cleanup.
- [x] Add generated short-audio test fixtures.
- [x] Add an integration test for model-input sample rate, channels, and Float32 format.

### Acceptance criteria

- WAV, FLAC, MP3, and M4A test inputs probe correctly on the development machine.
- Generated model input is exactly 44.1 kHz stereo Float32.
- No dither option is used for Float32 conversion.
- Paths containing Chinese characters work.

---

## Phase 10 — Compatibility residual output

### Tasks

- [x] Implement Float32 residual using FFmpeg with normalization disabled.
- [x] Add a focused test for the exact subtraction filter argument, including the unescaped
  `weights=1 -1` value.
- [x] Reuse the exact model-input WAV.
- [x] Implement FLAC encoding policy.
- [x] Implement and verify a single explicit triangular-dither step for 16/24-bit FLAC output.
- [x] Implement WAV Float32 output.
- [x] Implement partial-file output and FFprobe validation.
- [x] Implement atomic publication or safe replacement.
- [x] Implement output naming and conflict policies.
- [x] Add zero-vocals identity integration test.
- [x] Add full-vocals cancellation integration test.

### Acceptance criteria

- Zero-vocals test output matches input within Float32 tolerance.
- Full-vocals test output is near silence.
- Output is 44.1 kHz.
- No incomplete final file remains after simulated failure.
- Existing-file policy behaves as selected.

---

## Phase 11 — Experimental source sample rate output

### Tasks

- [x] Decode a source-native stereo Float32 WAV at the original sample rate.
- [x] Resample vocals to the source sample rate with SoXR precision 32.
- [x] Implement residual with source duration authority.
- [x] Implement source sample rate output validation.
- [x] Implement common source bit-depth mapping for FLAC.
- [x] Log duration and sample-rate comparison data.
- [x] Add explicit TODO references for alignment and clipping refinements.
- [x] Add short tests for 48 kHz and 96 kHz generated inputs.

### Acceptance criteria

- 48 kHz input produces 48 kHz experimental output.
- 96 kHz input produces 96 kHz experimental output.
- Compatibility output remains 44.1 kHz.
- Both outputs can be generated from one model inference.
- Experimental limitations are visible in Chinese UI copy.

---

## Phase 12 — Input planning, sequential batch runner, and real progress

### Tasks

- [x] Implement file and folder enumeration.
- [x] Implement recursion and output-directory exclusion.
- [x] Implement deterministic sorting.
- [x] Probe all inputs before processing.
- [x] Plan outputs and conflicts before processing.
- [x] Calculate total audio duration.
- [x] Implement duration-weighted overall progress.
- [x] Implement current-item stage progress.
- [x] Process strictly one item at a time.
- [x] Delete large temporary files after each item.
- [x] Continue after an individual item failure when safe.
- [x] Emit item and batch completion events.
- [x] Connect real backend progress to the existing GUI.

### Acceptance criteria

- A folder batch processes in deterministic order.
- Overall progress uses duration weighting.
- Completed outputs remain after a later item fails.
- Output directory inside input tree is not reprocessed.
- Temporary files do not accumulate across completed songs.

---

## Phase 13 — First-launch initialization integration

### Tasks

- [x] Connect first-launch environment status to the initialization flow.
- [x] Add `initialize_environment` and the first-use confirmation dialog.
- [x] Run the real initialization state machine before processing is enabled.
- [x] Reuse the progress dialog.
- [x] Enable batch submission only after successful initialization.
- [x] Implement retry after recoverable initialization failure.
- [x] Implement repair behavior for incomplete runtime state.

### Acceptance criteria

- First launch prompts before downloading and does not require the user to construct a batch first.
- Initialization shows overall and current progress.
- Successful initialization enables the normal extraction workflow.
- `start_batch` rejects an unready environment and never starts a hidden sync.
- Restarting the app with a ready runtime does not sync or redownload.
- Failed installation does not activate a broken runtime.

---

## Phase 14 — Error mapping, diagnostics, completion UX, and settings polish

### Tasks

- [x] Complete Rust domain error taxonomy.
- [x] Map worker errors.
- [x] Map FFmpeg/FFprobe failures.
- [x] Implement diagnostic report storage and copy action.
- [x] Implement localized recovery messages.
- [x] Implement final completion summary.
- [x] Implement failed-item list.
- [x] Persist safe user settings.
- [x] Add reveal-output-directory action.
- [x] Verify cancellation messaging and terminal states.

### Acceptance criteria

- CUDA unavailable and CUDA OOM show distinct messages.
- Raw traceback is absent from the normal UI.
- Diagnostic report includes enough technical information to investigate.
- Partial-success batch is presented correctly.
- Settings survive restart.

---

## Phase 15 — Core tests and smoke test documentation

Keep this phase intentionally small.

### Rust tests

- [x] Manifest parse and validation.
- [x] Hash verification success/failure.
- [x] Input enumeration and output exclusion.
- [x] Conflict naming.
- [x] Progress aggregation.
- [x] Cancellation transition.

### Python tests

- [x] Request validation.
- [x] JSON Lines serialization.
- [x] Worker configuration loading without loading the model.
- [x] Vendored MSST import-closure and patch-drift check.
- [x] Error-code mapping without loading the full model.

### Frontend tests

- [x] Reducer normal lifecycle.
- [x] Main-form validation.

### Audio integration tests

- [x] Model-input conversion properties.
- [x] Zero-vocals identity.
- [x] Full-vocals cancellation.
- [x] 48/96 kHz experimental output sample rate.
- [x] Output inspection for sample rate, channel count, codec, sample format, and raw bit depth.

### Documentation

- [x] Create `docs/SMOKE_TEST.md`.
- [x] Include Windows paths with spaces and Chinese characters.
- [x] Include cancellation and output-conflict cases.
- [x] Include a manual real-model GPU test.

### Acceptance criteria

- Core tests pass.
- Test suite remains fast without downloading the model.
- Full model inference is documented as a manual or separately triggered smoke test.

---

## Phase 16 — One-file packaging, licenses, and release build

### Tasks

- [x] Configure the Windows GUI subsystem so release builds do not open a console window.
- [x] Configure application icon and metadata.
- [x] Implement the deterministic bootstrap archive and `include_bytes!` integration from Section
  8.
- [x] Embed uv, the worker, the pinned and patched MSST snapshot, configuration, manifest, and
  licenses; do not configure them as Tauri `bundle.resources` or external sidecars.
- [x] Add build and extraction tests for entry hashes, path traversal, duplicate paths, size
  limits, reparse points, and corrupted payloads.
- [x] Ensure release manifest has no placeholders.
- [x] Include third-party license files.
- [x] Add an in-app open-source licenses view backed by the embedded/extracted notices.
- [x] Confirm FFmpeg build license obligations.
- [x] Confirm model attribution and revision.
- [x] Build the raw Windows executable with `pnpm tauri build --no-bundle`.
- [x] Copy only `soufmer.exe` to a clean directory and verify that no sibling file is required.
- [ ] Test the portable executable on a clean Windows user profile or VM with system WebView2.
- [ ] Test the native missing-WebView2 recovery path.
- [ ] Test first-run initialization without a global Python, Git, FFmpeg, or uv installation.
- [ ] Test from a read-only directory and a path containing spaces and Chinese characters.
- [ ] Verify all application-managed writes stay under `%LOCALAPPDATA%\soufmer\`, apart from the
  user-selected output directory.
- [ ] Start two executable copies concurrently and verify only one initializer mutates shared
  state.
- [x] Document portable removal: close the app, delete the executable, and optionally delete
  `%LOCALAPPDATA%\soufmer\` to remove the private runtime and settings.

### Portable release behavior

- Distribute exactly one `soufmer.exe`; do not produce an MSI/NSIS artifact for the MVP. Code
  signing remains a Phase 17 release-hardening task.
- Require no administrator privileges and never write beside the executable.
- Do not bundle the multi-gigabyte runtime.
- Use supported Windows' Evergreen WebView2 and the native prerequisite error described in Section
  8.
- Treat the executable and runtime as separately versioned: replacing the executable must neither
  delete nor blindly trust the existing private runtime.

### Acceptance criteria

- The release directory contains only `soufmer.exe`, and that file can be moved before launch.
- App starts without a console window.
- First-run runtime initialization succeeds on a clean profile and writes application state only
  beneath `%LOCALAPPDATA%\soufmer\`.
- The portable app processes a real audio file.
- No Git executable or network clone is needed.
- All required license notices are accessible.

---

## Phase 16A — End-to-end inference repair and UX stabilization

This is the next implementation priority. Complete it before the remaining Phase 16 clean-profile
release validation and before optional Phase 17 work.

### Recovery and reproduction

- [ ] Re-read `AGENTS.md` and this plan, then inspect `git status --short`, recent commits, the current
  diff, and relevant diagnostics before editing.
- [ ] Reproduce the current `INFERENCE_FAILED` with one short legal audio fixture and record the exact
  worker request, failing boundary, stable error, exit code, and bounded stderr in diagnostics.
- [ ] Confirm whether verbatim path prefixes enter FFprobe, FFmpeg, worker request JSON, Python
  validation, MSST/SoundFile loading, or more than one boundary. Do not assume the first visible
  prefix is the only defect.

### Windows path normalization repair

- [ ] Add one narrowly scoped Rust path-boundary helper or type that converts validated canonical
  paths to external-process paths according to Section 12.
- [ ] Convert `\\?\UNC\server\share\path` to `\\server\share\path` and `\\?\C:\path` to
  `C:\path` only after canonical containment/security checks.
- [ ] Apply the helper consistently to FFprobe, FFmpeg, uv/Python process arguments, worker request
  JSON, and any downstream library-facing path generated by Rust.
- [ ] Keep user-facing paths readable and keep canonical/verbatim forms internal to filesystem
  validation and diagnostics.
- [ ] Add focused tests for drive paths, UNC paths, already-normal paths, spaces, Simplified Chinese,
  the regression form `\\?\UNC\snas.local\WD\Media\Record\...`, malformed prefixes, device
  namespaces, and unsupported long-path cases.
- [ ] Add or update a stable localized `PATH_UNSUPPORTED` mapping for paths that cannot safely cross a
  required process boundary.
- [ ] Run one successful real-model GPU inference from a normal local path.
- [ ] Run one successful real-model GPU inference from a UNC share when a test share is available;
  otherwise document the blocked manual test and keep the focused UNC conversion test mandatory.

### Initialization progress and activity UX

- [ ] Add the `runtime://activity` payload and frontend subscription described in Sections 7 and 11.
- [ ] Capture uv output incrementally with ANSI color disabled, preserve bounded raw diagnostics, and
  emit only sanitized recognized activity messages to the normal UI.
- [ ] Show a concise current-activity line and a bounded recent-activity `ScrollArea` during
  initialization. Keep automatic scrolling unless the user intentionally reviews older entries.
- [ ] Use reliable byte totals or completed/total unit counts when available. Keep the current-step
  bar indeterminate when no trustworthy denominator exists.
- [ ] Keep the stage-weighted overall bar monotonic; do not infer percentage from elapsed time or uv's
  cosmetic spinner.
- [ ] Add a reassuring long-operation message for the CUDA/Torch environment sync and keep elapsed
  time updating even when uv emits no new line.
- [ ] Add focused reducer/event tests for activity sequencing, stale-task rejection, bounded history,
  and determinate/indeterminate transitions.

### Final visual theme

- [ ] Replace the blue shadcn/Tailwind primary and progress tokens with the shared magenta palette in
  Section 19.1.
- [ ] Use `#FF8AD8` for bright display accents and progress fills, and an accessible darker primary
  such as `#C12B8F` for filled buttons with white text.
- [ ] Apply consistent hover, pressed, selected, disabled, and focus-visible states without changing
  destructive/warning/success semantics.
- [ ] Verify normal text contrast at 4.5:1 where applicable and control/focus indicators at 3:1.
- [ ] Confirm there are no accidental blue primary/progress treatments left in the normal MVP UI.

### Main-window fit and visibility

- [ ] Refactor the main page into a full-height layout with page-level overflow hidden.
- [ ] Keep environment status and initialization/start actions visible in a persistent bottom region.
- [ ] Keep advanced options collapsed by default and remove unnecessary vertical gaps.
- [ ] Measure the rendered layout and set the Tauri initial/minimum window size so the normal collapsed
  page fits at 100%, 125%, and 150% Windows scaling without a page scrollbar.
- [ ] At 175% and 200%, preserve every control and visible keyboard focus; use a bounded internal
  scroll region only when the monitor work area cannot contain the whole optional content region.
- [ ] Verify resizing does not cover the focused control, truncate essential localized text, or move
  the environment action below an unreachable area.

### Validation and commits

- [ ] Commit the path repair with its focused tests as one independently reviewable unit.
- [ ] Commit initialization activity/progress improvements with their protocol and frontend tests as
  one independently reviewable unit.
- [ ] Commit theme and window-fit polish as one independently reviewable UI unit.
- [ ] Run frontend lint, typecheck, tests, and build.
- [ ] Run Rust formatting, clippy, and tests.
- [ ] Run the locked lightweight worker tests.
- [ ] Build the raw standalone executable and run a portable smoke test that initializes the runtime
  if needed and successfully processes one real audio file.
- [ ] Update this phase, Phase 2, Phase 8, Phase 16, the release checklist, and the Decision Log in the
  same commits as the verified results they describe.

### Acceptance criteria

- A real audio file completes model inference and publishes a valid accompaniment output instead of
  returning `INFERENCE_FAILED`.
- No normal worker request or FFmpeg/FFprobe/Python argument contains a `\\?\` or
  `\\?\UNC\` prefix; unsupported exceptional paths fail early with a specific localized error.
- Initialization step 4 visibly communicates current activity and elapsed time. Determinate values
  are backed by measured bytes or units, and indeterminate work is not represented by a fake percent.
- The normal collapsed main screen has no page-level scrollbar at 100%, 125%, and 150% scaling, and
  environment initialization/start actions remain visible.
- The UI uses the shared magenta theme, retains semantic status colors, and passes the specified
  contrast and keyboard-focus checks.
- The standalone executable still builds without required sidecars and completes a real local-path
  audio-processing smoke test.

---

## Phase 17 — Release hardening and deferred quality work

This phase is not required to prove the MVP but should be completed before broad public release.

### Tasks

- [ ] Sign the Windows executable.
- [ ] Define update strategy for GUI, runtime, model, and FFmpeg independently.
- [ ] Define how a user downloads and replaces a portable executable without self-modification.
- [ ] Add rollback to previous runtime after failed update.
- [ ] Verify download behavior behind common proxies and restricted networks.
- [ ] Profile disk usage and installation peak usage.
- [ ] Profile GPU memory and set a safe default batch size.
- [ ] Add one automatic retry with batch size 1 for CUDA OOM if the initial default is larger.
- [ ] Measure source-rate alignment across sample-rate pairs.
- [ ] Decide clipping and limiting policy.
- [ ] Decide metadata and cover-art copying policy.
- [ ] Add crash recovery for abandoned job directories.

---

## 24. Core test designs

## 24.1 Zero-vocals identity

Generate a short stereo Float32 test signal.

Inputs:

- Mixture: generated sine mixture.
- Vocals: all zeros of matching length.

Expected:

- Residual closely matches mixture.
- Sample rate and channels match.
- Maximum absolute sample error is below a justified tolerance.

## 24.2 Full-vocals cancellation

Inputs:

- Mixture: generated sine mixture.
- Vocals: exact copy of mixture.

Expected:

- Residual RMS and peak are near zero.
- Detect accidental FFmpeg normalization or integer conversion.

## 24.3 Progress aggregation

Batch:

- Song A: 120 seconds, complete.
- Song B: 7,200 seconds, 10% complete.

Expected overall fraction:

```text
(120 + 7200 × 0.10) / 7320
```

Test the exact implemented calculation and clamping.

## 24.4 Output exclusion

Input folder contains the selected output folder.

Expected:

- Output folder is excluded.
- Generated files from a previous run are not included in the input snapshot.

## 24.5 Hash mismatch

Expected:

- Downloaded artifact is not activated.
- Stable error is `ENV_HASH_MISMATCH`.
- Existing working runtime remains active.

---

## 25. Manual smoke-test matrix

| Input | Compatibility | Source SR | Notes |
|---|---:|---:|---|
| 44.1 kHz stereo FLAC | Required | Required | Baseline |
| 48 kHz stereo FLAC | Required | Required | Verify 44.1 vs 48 output |
| 96 kHz stereo FLAC | Required | Required | Verify 96 output |
| 44.1 kHz stereo MP3 | Required | Required | Encoding-delay risk |
| 48 kHz M4A/AAC | Required | Required | Container timing risk |
| Mono WAV | Required | Required | Verify controlled stereo duplication and stereo output |
| Chinese filename/path | Required | Required | Windows path handling |
| Very long path | Required | Optional | Confirm current Windows settings |
| Existing output | Required | Required | Skip/overwrite/number |
| Output inside input tree | Required | Required | Exclusion |
| Cancel during inference | Required | Required | Process-tree cleanup |
| Cancel during download | Required | N/A | Resume or safe cleanup |
| CUDA unavailable | Required | Required | Friendly error |
| CUDA OOM | Required | Required | Stable diagnostic |

---

## 26. Quality-gate commands

Run from repository root unless noted.

### Frontend

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

### Rust

```powershell
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

### Worker

During development with uv installed:

```powershell
# Fast tests keep model imports lazy and do not require the CUDA extra.
uv lock --project worker --check
uv run --project worker --locked pytest worker/tests

# Run separately when validating the installed CUDA/model path.
uv run --project worker --locked --extra cuda python -m accompaniment_worker self-test
```

### Application

```powershell
pnpm tauri dev
pnpm tauri build --no-bundle
```

For a release checkpoint, copy only `src-tauri/target/release/soufmer.exe` to an otherwise empty
directory and run the clean-profile portable smoke test. Fail the release if the executable needs
a repository file, Tauri `$RESOURCE` directory, DLL shipped beside it, or write access to its own
directory. Normal Windows system DLLs and the system Evergreen WebView2 runtime are not sidecars.

Do not run the full model smoke test as part of the normal fast test command.

---

## 27. Suggested initial dependencies

Do not add all dependencies before their phase.

### Frontend

Core:

- `react`
- `react-dom`
- `i18next`
- `react-i18next`
- `zod`
- `lucide-react`

Development and styling:

- `typescript`
- `vite`
- `@vitejs/plugin-react`
- `tailwindcss`
- `@tailwindcss/vite`
- `vitest`
- ESLint packages selected by the scaffold

shadcn will add component-specific dependencies as required.

### Tauri frontend bindings

- `@tauri-apps/api`
- `@tauri-apps/plugin-dialog`
- `@tauri-apps/plugin-opener`

### Rust

Add by phase:

- `serde`
- `serde_json`
- `tokio`
- `thiserror`
- `tracing`
- `tracing-subscriber`
- `uuid`
- `directories`
- `walkdir`
- `reqwest`
- `sha2`
- `zip`
- `chrono` or `time`, choose one
- `windows` or `windows-sys` for Job Objects
- Tauri plugins used by the frontend

Avoid adding both competing implementations for the same concern.

### Python

The exact list is determined by the pinned MSST snapshot and model. The initial audited set is
documented in Section 13, including the required `einops`, `beartype`, and
`rotary-embedding-torch` imports. Keep the final list in `worker/pyproject.toml` and
`worker/uv.lock`; never document an informal post-sync `pip install` sequence as a supported setup.

Torch and torchaudio belong to the explicit `cuda` extra and fixed PyTorch index. Training, GUI,
plotting, streaming, optimizer, and unrelated-model packages from upstream MSST are not product
dependencies.

---

## 28. Documentation deliverables

### `README.md`

Keep it user/developer oriented:

- What the project does.
- Supported platform.
- Basic development setup.
- How to run Tauri dev.
- How to prepare runtime manifest values.
- License summary.

### `docs/ARCHITECTURE.md`

- Component diagram.
- Runtime layout.
- IPC boundary.
- State machines.
- Cancellation model.

### `docs/RUNTIME_MANIFEST.md`

- Field descriptions.
- Hash-generation procedure.
- Release checklist.
- Development override procedure.

### `docs/AUDIO_PIPELINE.md`

- Compatibility pipeline.
- Source-rate pipeline.
- SoXR precision 32 decision.
- Dither policy.
- Known TODOs.

### `docs/LICENSING.md`

- Project MIT license.
- MSST MIT obligations.
- Model attribution and license record.
- FFmpeg build license.
- Generated third-party notices process.

### `docs/SMOKE_TEST.md`

- Short manual checklist.
- Expected outputs.
- Diagnostic collection instructions.

---

## 29. Release manifest preparation checklist

Before producing a public build:

- [x] Select uv binary version.
- [x] Verify uv binary source.
- [x] Calculate uv SHA-256.
- [x] Verify the committed lockfile with the exact uv binary embedded for release.
- [x] Select FFmpeg build.
- [x] Verify FFmpeg license type.
- [x] Record FFmpeg version.
- [x] Calculate archive SHA-256.
- [x] Pin model Hugging Face revision.
- [x] Archive the model card/license state from that exact revision.
- [x] Calculate model SHA-256.
- [x] Pin MSST commit.
- [x] Verify vendored file hashes, upstream MIT text, and ordered patch set.
- [x] Select and record the fixed PyTorch CUDA index plus matching Torch/torchaudio versions.
- [x] Regenerate `uv.lock` after dependency decisions.
- [ ] Perform a clean locked CUDA sync and confirm a second sync changes nothing.
- [ ] Confirm the self-test reports the expected CUDA build suffix and runs a tiny CUDA operation.
- [ ] Run worker self-test from a clean `%LOCALAPPDATA%\soufmer\` runtime.
- [ ] Replace conservative download/install/disk estimates with measured release values.
- [x] Confirm no manifest placeholder remains.
- [x] Confirm estimated download and disk size displayed by the UI.

---

## 30. Deferred audio TODO register

Keep these visible in `docs/AUDIO_PIPELINE.md` and tracked issues.

### AUDIO-001 — Exact source-rate alignment

Measure sample offset introduced by:

- Source decoder delay.
- Source-to-44.1 kHz conversion.
- Model inference framing.
- 44.1 kHz-to-source-rate conversion.

Add deterministic correction only after measurement.

### AUDIO-002 — Sample-count enforcement

Implement exact trim/pad policy based on decoded source sample count rather than container duration.

### AUDIO-003 — Clipping strategy

Measure residual peaks and select one of:

- Preserve float over-range for Float32 WAV.
- Hard clip for integer encodes.
- Optional transparent limiter.
- Optional peak normalization.

Do not silently normalize in the MVP.

### AUDIO-004 — Metadata and cover art

Define safe mapping for FLAC, WAV, MP3, and M4A outputs.

### AUDIO-005 — Mono behavior

The MVP duplicates mono sources to the required stereo model input and produces stereo outputs.
Measure whether a later release can safely restore mono output without discarding useful
channel-different model estimates; if so, add it as an explicit option or documented policy.

---

## 31. Definition of MVP done

The MVP is done when all of the following are true:

- One movable `soufmer.exe` is the complete application distribution artifact for Windows 10/11
  x64.
- The app opens without a command prompt window.
- The UI defaults to Simplified Chinese.
- All source code and developer-facing content are English.
- A user can choose one file or a folder and an output directory.
- Compatibility mode is the default.
- Experimental source sample rate mode is available and labeled.
- First launch initializes the private runtime under `%LOCALAPPDATA%\soufmer\` with two-level
  progress and no Git dependency.
- Later starts do not rerun environment sync.
- One real KimberleyJSN model inference succeeds on an NVIDIA GPU.
- Batch files are processed sequentially.
- Overall and current progress update.
- Cancellation terminates child processes and preserves completed songs.
- Temporary files are deleted per item.
- Compatibility output and source-rate output pass property checks.
- Core automated tests pass.
- License notices for the embedded uv/MSST components, model, and FFmpeg are included.
- A clean portable smoke test proves that no required file is shipped beside the executable and no
  application-managed write occurs beside it.

---

## 32. Decision log

Record deviations in this format:

```text
YYYY-MM-DD — Decision title
Context:
Decision:
Consequences:
```

Initial decisions:

```text
2026-07-30 — Use online runtime bootstrap
Context: The local Python/CUDA/model environment is approximately 3.6 GB.
Decision: Ship one portable executable with a small hash-verified bootstrap payload embedded in the
binary, then install large private runtime components on first launch.
Consequences: Initialization requires reliable progress, download verification, safe embedded
payload extraction, and recovery. MSI/NSIS resources and required sidecars cannot be used.
```

```text
2026-07-30 — Use one fixed application data root
Context: The portable executable may be stored in or launched from any directory.
Decision: Put every application-managed writable file under the exact
%LOCALAPPDATA%\soufmer\ root. User-selected inputs and outputs remain user-controlled exceptions.
Consequences: The executable directory and current working directory are never storage roots;
multiple executable copies share versioned runtime state and an initialization mutex.
```

```text
2026-07-30 — Use compatibility mode by default
Context: Residual subtraction is most stable when the exact 44.1 kHz model input is reused.
Decision: Compatibility mode is default; source sample rate mode is experimental.
Consequences: Default output is 44.1 kHz, while advanced users may retain source sample rate.
```

```text
2026-07-30 — Use SoXR precision 32
Context: The project wants a consistent high-quality resampling configuration.
Decision: All resampling uses SoXR precision 32. Float32 stages do not use dither.
Consequences: Final integer conversion applies triangular dither once.
```

```text
2026-07-30 — Keep automated tests focused
Context: The project does not require broad unit-test coverage.
Decision: Test only core state, path, manifest, protocol, cancellation, and audio invariants.
Consequences: Visual behavior and real model inference rely on a concise manual smoke checklist.
```

```text
2026-07-30 — Make the locked uv project authoritative for CUDA
Context: Repairing Torch with `uv pip install --torch-backend=auto` after `uv sync` leaves the
environment different from `pyproject.toml` and `uv.lock`; a later `uv run` can restore the locked
Windows/PyPI Torch build. Required RoFormer packages were also being added manually.
Decision: Declare the audited MSST dependencies in `worker/pyproject.toml`, put version-matched
Torch and torchaudio in a `cuda` extra mapped to one fixed explicit PyTorch index, and install only
with locked project commands.
Consequences: No normal workflow uses `uv pip install --reinstall` or `--no-sync`. A new CUDA
backend requires a deliberate dependency/lock/runtime-profile update and a clean GPU validation.
```

```text
2026-08-01 — Select a SoXR-enabled BtbN FFmpeg build
Context: The initially selected Gyan.D 8.0.1 essentials ZIP does not include libsoxr and failed
the required SoXR model-input conversion gate.
Decision: Pin BtbN build n8.0.1-66-g27b8d1a017-20260228 from the immutable monthly release tag
autobuild-2026-02-28-12-59. Use the static LGPL-3.0 ZIP whose configuration reports
--enable-libsoxr, and bump the bootstrap compatibility version so the rejected runtime cannot be
reused.
Consequences: The verified generated-audio gate covers model conversion, identity and cancellation
residuals, source-rate output, and output inspection. BtbN targets Windows 10 22H2 and newer, and
its two-year monthly-release retention requires repinning before February 2028.
```

```text
2026-08-01 — Normalize canonical Windows paths at process boundaries
Context: Rust canonicalization can produce verbatim paths such as \\?\UNC\server\share\path, and the
current real-audio path reaches model inference in that form and fails with INFERENCE_FAILED.
Decision: Keep canonical/verbatim paths for internal validation, but derive a separate normal drive or
UNC representation after validation and before process arguments or worker JSON serialization.
Consequences: Local and UNC path conversions require focused regression tests. Unsupported long or
device paths fail early with PATH_UNSUPPORTED instead of a generic inference error.
```

```text
2026-08-01 — Show structured initialization activity
Context: The environment-sync step downloads and installs large CUDA components and can remain
indeterminate for a long time, while a bouncing bar alone does not reassure users that work continues.
Decision: Keep truthful determinate/indeterminate progress semantics and add a bounded sanitized
activity feed plus current activity and elapsed time. Raw terminal output remains diagnostic-only.
Consequences: The backend adds runtime://activity, uv output is captured incrementally with ANSI color
disabled, and the UI never invents percentages or unstable remaining-time estimates.
```

```text
2026-08-01 — Use an accessible Final Cut-inspired magenta theme
Context: The scaffold's blue controls do not match the intended product identity, while the requested
HSB 320°, 46%, 100% pink is too light for normal white button text.
Decision: Use #FF8AD8 as a bright display accent and use a darker magenta such as #C12B8F for filled
controls with white text. Keep semantic error, warning, and success colors distinct.
Consequences: Shared theme tokens replace component-local blue styling, and contrast/focus checks are
part of Phase 16A acceptance.
```

```text
2026-08-01 — Keep primary actions visible without normal page scrolling
Context: The environment status and initialization action can fall below the initial viewport and be
missed in a simple single-screen utility.
Decision: Use a measured full-height layout with a persistent bottom action region and no page-level
scrollbar at normal Windows scaling. Allow a bounded internal fallback only for optional content at
extreme accessibility scaling when the monitor work area cannot fit everything.
Consequences: The window minimum size, spacing, localization, and 100%-200% scaling behavior require
manual verification.
```

---

## 33. Official implementation references

Consult current official documentation before copying setup commands:

- Tauri project creation: https://v2.tauri.app/start/create-project/
- Tauri Vite configuration: https://v2.tauri.app/start/frontend/vite/
- Tauri dialog plugin: https://v2.tauri.app/plugin/dialog/
- Tauri command IPC: https://v2.tauri.app/develop/calling-rust/
- Tauri frontend events/channels: https://v2.tauri.app/develop/calling-frontend/
- Tauri build and `--no-bundle`: https://v2.tauri.app/distribute/
- Tauri additional resources behavior: https://v2.tauri.app/develop/resources/
- Tauri Windows WebView2 guidance: https://v2.tauri.app/distribute/windows-installer/#webview2-installation-options
- Rust `include_bytes!`: https://doc.rust-lang.org/std/macro.include_bytes.html
- Windows `SHGetKnownFolderPath`: https://learn.microsoft.com/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath
- shadcn Vite installation: https://ui.shadcn.com/docs/installation/vite
- Tailwind Vite installation: https://tailwindcss.com/docs/installation/using-vite
- Vite requirements: https://vite.dev/guide/
- pnpm installation and compatibility: https://pnpm.io/installation
- uv PyTorch guide: https://docs.astral.sh/uv/guides/integration/pytorch/
- uv locking and syncing: https://docs.astral.sh/uv/concepts/projects/sync/
- uv resolution environments: https://docs.astral.sh/uv/concepts/resolution/
- uv Python versions and managed-Python selection: https://docs.astral.sh/uv/concepts/python-versions/
- uv storage locations and overrides: https://docs.astral.sh/uv/reference/storage/
- FFmpeg `amix` filter: https://ffmpeg.org/ffmpeg-filters.html#amix
- FFmpeg resampler and dither options: https://ffmpeg.org/ffmpeg-resampler.html
- MSST repository: https://github.com/ZFTurbo/Music-Source-Separation-Training
- MSST dependency metadata: https://github.com/ZFTurbo/Music-Source-Separation-Training/blob/main/pyproject.toml
- Kimberley MSST configuration: https://github.com/ZFTurbo/Music-Source-Separation-Training/blob/main/configs/KimberleyJensen/config_vocals_mel_band_roformer_kj.yaml
- Kimberley model repository: https://huggingface.co/KimberleyJSN/melbandroformer
- Windows progress controls: https://learn.microsoft.com/windows/apps/develop/ui/controls/progress-controls
- Windows maximum path and verbatim-prefix behavior: https://learn.microsoft.com/windows/win32/fileio/maximum-file-path-limitation
- uv CLI progress and color options: https://docs.astral.sh/uv/reference/cli/
- WCAG 2.2 contrast requirements: https://www.w3.org/TR/WCAG22/
- WCAG non-text contrast guidance: https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast
- WCAG focus appearance guidance: https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance
