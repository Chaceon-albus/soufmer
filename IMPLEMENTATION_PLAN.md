# Accompaniment Extractor — Implementation Plan

Status: Main implementation complete; maintenance and owner-directed refinement
Last reviewed: 2026-08-02
Target platform: Windows 10/11 x64
Default UI locale: Simplified Chinese (`zh-CN`)
Development language: English
Primary stack: Tauri 2, Rust, React, TypeScript, Vite, Tailwind CSS, shadcn/ui, and Python managed by uv

## 1. Purpose and current state

This document is the compact architectural and maintenance plan for the repository. Detailed coding,
Git, delegation, and validation rules belong in `AGENTS.md`.

The implemented application:

- Builds as one movable `soufmer.exe` with no required sidecar files.
- Initializes a private AI runtime under `%LOCALAPPDATA%\soufmer\` on first use.
- Uses a pinned Mel-Band RoFormer model through a vendored and locally adapted MSST inference path.
- Processes one file or a folder sequentially.
- Produces accompaniment output in compatibility or source-sample-rate mode.
- Shows initialization and processing progress through the desktop UI.
- Uses localized Simplified Chinese UI text with an English fallback.
- Uses a shared magenta visual theme and a main layout that keeps primary actions visible.
- Converts validated Windows canonical paths to downstream-compatible process paths before invoking
  FFmpeg, FFprobe, uv, or Python.

The project owner has manually confirmed that the standalone application starts, initializes the AI
environment, and successfully separates audio on the current development machine.

Manual and release testing are managed by the project owner and are intentionally not tracked as
unfinished checklist items in this plan.

## 2. Product behavior

### 2.1 User workflow

1. Launch `soufmer.exe`.
2. Initialize or repair the private runtime when required.
3. Select one file or a folder.
4. Select an output directory and processing options.
5. Process files sequentially.
6. Review completion or localized error details.

The normal UI must not expose command construction, raw tracebacks, or unrestricted terminal output.

### 2.2 Processing modes

**Compatibility mode — default**

- Convert the selected audio stream to 44.1 kHz stereo Float32 using SoXR precision 32.
- Run model inference against that exact model-input file.
- Subtract the vocals estimate from the same model-input file.
- Produce a 44.1 kHz result.

**Source sample rate mode — experimental**

- Decode the source to stereo Float32 at its native sample rate.
- Resample the 44.1 kHz vocals estimate to the source sample rate with SoXR precision 32.
- Subtract using the source signal as the duration authority.
- Preserve common source bit depths where supported.
- Keep exact alignment, clipping policy, and advanced metadata behavior as deferred work.

### 2.3 Output behavior

- Default output format: FLAC.
- Optional output format: Float32 WAV.
- Default filename: `<source stem> (Instrumental).<extension>`.
- When both modes are generated:
  - `<source stem> (Instrumental - 44.1k).<extension>`
  - `<source stem> (Instrumental - Source SR).<extension>`
- Conflict policies: `skip`, `overwrite`, or `autoNumber`.
- Final output is first written as a partial file, validated with FFprobe, and then published.
- Temporary files for the current item are removed after success, failure cleanup, or cancellation.

## 3. Fixed architecture

| Area | Decision |
|---|---|
| Desktop framework | Tauri 2 |
| Frontend | React + TypeScript + Vite |
| Styling | Tailwind CSS with shadcn/ui |
| Localization | i18next + react-i18next |
| Frontend state | `useReducer` with discriminated unions |
| Backend | Rust commands and typed events |
| Worker | Python CLI with JSON request and JSON Lines output |
| Runtime manager | Embedded uv with private `%LOCALAPPDATA%\soufmer\` runtime |
| Model | KimberleyJSN MelBandRoformer |
| Inference implementation | Pinned and modified MSST snapshot |
| Audio tools | Fixed FFmpeg/FFprobe build with libsoxr |
| Resampling | SoXR precision 32 |
| Processing | Sequential, one song at a time |
| Distribution | One portable `soufmer.exe` |
| WebView | System Evergreen WebView2 |

### 3.1 Responsibility boundaries

**React frontend**

- Collects user options and paths.
- Displays environment state, progress, completion, and localized errors.
- Does not build shell commands or directly execute external programs.

**Rust/Tauri backend**

- Owns validation, runtime state, downloads, extraction, process creation, cancellation, planning,
  progress aggregation, diagnostics, and final output publication.
- Validates all frontend payloads.
- Invokes external programs with argument arrays and never through shell-concatenated command text.

**Python worker**

- Loads the pinned model and configuration.
- Processes exactly one controlled 44.1 kHz stereo Float32 input per invocation.
- Emits JSON Lines protocol messages.
- Writes one controlled vocals result into the assigned job directory.
- Does not enumerate user folders, choose arbitrary output paths, download dependencies, or perform
  final encoding.

**FFmpeg/FFprobe**

- Inspect source and output media.
- Create the controlled model input.
- Perform Float32 residual calculation and final encoding.

## 4. Repository structure

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
├─ tests/
│  └─ fixtures/
└─ docs/
   ├─ ARCHITECTURE.md
   ├─ RUNTIME_MANIFEST.md
   ├─ AUDIO_PIPELINE.md
   ├─ LICENSING.md
   └─ SMOKE_TEST.md
```

The structure may be simplified when a module would contain only trivial forwarding code. Do not
create unused abstraction layers solely to match the tree.

## 5. Runtime and distribution contract

### 5.1 One-file bootstrap

The release artifact is the raw executable produced by:

```powershell
pnpm tauri build --no-bundle
```

Frontend assets and the deterministic bootstrap archive are compiled into the executable. The embedded
bootstrap contains uv, the worker project, the vendored MSST subset, configuration, runtime manifest,
and license notices. Python, Torch, torchaudio, FFmpeg, and the model checkpoint are downloaded during
private runtime initialization.

Required bootstrap properties:

- Deterministic archive input ordering and normalized paths.
- Per-entry hashes and an outer archive digest.
- Rejection of missing licenses, placeholder manifest values, path escapes, links/reparse points,
  duplicates, and absolute developer paths.
- No Git/VCS dependency or mutable MSST source in `worker/uv.lock`.

### 5.2 Private application data

All application-managed writable data must remain under:

```text
%LOCALAPPDATA%\soufmer\
```

```text
%LOCALAPPDATA%\soufmer\
├─ bootstrap/versions/<bootstrap-id>/
├─ state/
├─ runtime/versions/<runtime-id>/
├─ tools/ffmpeg/<version>/
├─ models/kimberley-melbandroformer/<revision>/
├─ downloads/
├─ staging/
├─ cache/
├─ jobs/
├─ logs/
└─ diagnostics/
```

User-selected input and output paths are the only intentional exceptions. The executable directory,
current working directory, `%TEMP%`, global Python locations, and global PATH must not become product
storage roots.

### 5.3 Runtime activation

A runtime is active only after its files, manifest digest, profile, self-test record, and `READY` marker
are valid. Activation is an atomic update of `current-runtime.json`; incomplete candidates must never
replace an existing working runtime.

Multiple executable copies share the same runtime root and initialization mutex.

### 5.4 Dependency policy

- Developer toolchain: stable Rust supported by Tauri 2, Node.js 22.12 or newer, and pnpm 11 or newer.
- Commit `pnpm-lock.yaml`, `Cargo.lock`, and `worker/uv.lock`.
- End-user Python minor version remains 3.11.
- Torch and torchaudio belong to the locked `cuda` extra and one explicit PyTorch CUDA index.
- `worker/pyproject.toml` and `worker/uv.lock` are authoritative.
- Do not repair the managed environment with ad hoc `uv pip install`, repeated `uv add`, or normal
  `--no-sync` workflows.
- Production audio jobs execute the validated private Python directly and do not use `uv run`.

## 6. Backend API and protocols

### 6.1 Main commands

- `get_environment_status`
- `initialize_environment`
- `get_app_settings`
- `save_app_settings`
- `validate_batch_request`
- `start_batch`
- `cancel_active_task`
- `get_diagnostic_report`
- `reveal_output_directory`

The frontend must not supply executable paths, URLs, package names, or command text.

### 6.2 Events

```text
runtime://progress
runtime://activity
batch://progress
batch://item-completed
batch://completed
task://failed
task://cancelled
```

Each task event uses a versioned envelope with task ID, monotonically increasing sequence number,
timestamp, type, and payload. The frontend ignores stale task IDs and non-increasing sequence numbers.

`runtime://activity` is informational. Progress and terminal events remain authoritative.

### 6.3 Worker protocol

Conceptual invocation:

```powershell
<private-python.exe> -I -m accompaniment_worker separate --request <request.json>
```

The worker request contains controlled input, output, checkpoint, configuration, device, batch-size,
and overlap values. Worker stdout is JSON Lines only. Python logging and tracebacks go to stderr and
are stored in diagnostics rather than shown directly in the normal UI.

Stable worker failures distinguish invalid request, runtime import failure, model load failure, CUDA
unavailable, CUDA out of memory, inference failure, output failure, and cancellation.

## 7. Windows path boundary

Rust may use canonical or verbatim paths internally for identity, containment, and security checks.
These forms must not be passed unchanged to downstream tools unless explicitly supported.

After validation and immediately before JSON serialization or argument construction:

- `\\?\C:\path` becomes `C:\path`.
- `\\?\UNC\server\share\path` becomes `\\server\share\path`.
- Normal drive and UNC paths remain unchanged.
- Device namespaces, malformed prefixes, volume GUID paths, and unsupported long-path cases are
  rejected with a stable localized `PATH_UNSUPPORTED` error.

Keep canonical internal paths, display paths, and process/worker paths conceptually separate. Do not
perform blind string replacement on unvalidated user input.

## 8. Audio pipeline

### 8.1 Source inspection

FFprobe is authoritative. Record the selected audio stream, codec, sample rate, channel count, channel
layout, duration, sample format, bit depth, and container. The initial extension prefilter includes:

```text
.wav .flac .mp3 .m4a .aac .ogg .opus .aiff .aif .wma
```

Reject files with no usable audio stream, more than two channels, invalid duration, or unsupported
content.

### 8.2 Model input

Create one controlled WAV per item:

- 44,100 Hz.
- Stereo.
- Float32 PCM.
- SoXR precision 32.
- No dither.

The logical FFmpeg filter is:

```text
aresample=resampler=soxr:osr=44100:precision=32
```

### 8.3 Residual and encoding

Compatibility residual uses the exact model-input WAV and the 44.1 kHz vocals result:

```text
[0:a:0][1:a:0]amix=inputs=2:duration=first:dropout_transition=0:weights=1 -1:normalize=0[out]
```

Float32 intermediate signals are not dithered. When converting to integer PCM for FLAC, apply one
explicit triangular-dither step. Float32 WAV uses `pcm_f32le` without dither.

A final output is published only after FFmpeg succeeds and FFprobe confirms the expected audio stream,
sample rate, channels, duration tolerance, codec, and sample format.

### 8.4 Batch planning

- Enumerate supported files deterministically.
- Optionally recurse and preserve directory structure.
- Exclude an output directory located inside the input tree.
- Take a complete input snapshot before processing.
- Probe and plan outputs before model inference.
- Process one item at a time.
- Preserve completed outputs when a later item fails or the batch is cancelled.

## 9. Initialization and progress UX

Initialization proceeds through:

1. Check system and storage.
2. Validate or extract the embedded bootstrap and prepare FFmpeg.
3. Install private Python 3.11.
4. Synchronize the locked CUDA environment.
5. Download and verify the model.
6. Run the private runtime self-test.
7. Activate the validated runtime.

Progress rules:

- Use determinate progress only for measured bytes or reliable completed/total units.
- Keep unmeasurable work indeterminate; never derive percentages from elapsed time or a spinner.
- Keep overall progress monotonic and stage-weighted.
- During long environment synchronization, show elapsed time, a concise current-activity message, and
  a bounded sanitized activity feed.
- Strip ANSI control sequences and never expose command lines, credentials, raw tracebacks, or an
  unrestricted terminal transcript.
- Preserve complete bounded process output in diagnostics.

The same progress surface is reused for initialization and audio processing.

## 10. Frontend behavior

### 10.1 Main window

- One Tauri window with an in-window progress dialog.
- Full-height layout with no normal page-level scrollbar.
- Environment status and the primary initialization/start action remain visible in a persistent bottom
  region.
- Advanced options are collapsed by default.
- Optional content may use a bounded internal scroll region only when the available work area cannot
  contain it.

### 10.2 Visual system

Use shared Tailwind/shadcn theme tokens:

- Bright display accent and progress: `#FF8AD8`.
- Primary filled controls: `#C12B8F` with white text.
- Hover or pressed primary state: approximately `#A61E73`.
- Soft selected background: approximately `#FFF0FA` with dark text.
- Preserve semantic error, warning, success, and neutral colors.

Do not scatter component-local color literals when a shared token is appropriate.

### 10.3 Main controls

- File/folder selector.
- Input path and browse action.
- Output directory and browse action.
- Compatibility/source-rate mode cards.
- Collapsible advanced options.
- Environment status.
- Primary extraction action.

Advanced options include recursion, relative-directory preservation, output format, conflict policy,
and optional generation of both processing modes.

### 10.4 State model

The top-level application lifecycle remains a discriminated union covering booting, idle, validation,
initialization consent, initialization, processing, cancellation, completion, and failure. Invalid or
stale transitions must not corrupt active state.

## 11. Process control, security, and diagnostics

- All external processes use one controlled process abstraction.
- Executables and arguments are passed separately; no `cmd.exe /C`, PowerShell command text, or shell
  escaping is used.
- Child windows remain hidden.
- Windows Job Objects own child process trees so cancellation and application exit terminate
  descendants.
- Cancellation checks occur before and after process boundaries, during downloads, before publication,
  and between items.
- Runtime manifests are trusted configuration; downloaded archives remain untrusted until verified.
- Archive extraction rejects traversal, absolute paths, links/reparse points, duplicates, alternate
  data streams, and entries outside declared limits.
- Frontend Tauri permissions remain narrow; no generic shell capability is exposed.
- Logs use English structured fields and remain beneath the private data root.
- Diagnostic reports may include versions, stages, paths, exit codes, probe summaries, and bounded
  stderr, but never authorization headers or credentials.

## 12. Vendored MSST policy

- Pin one exact upstream commit.
- Record repository URL, full commit, retrieval date, copied-file inventory, and source hashes.
- Vendor only the import closure required for the selected Mel-Band RoFormer inference path.
- Preserve the upstream MIT license and copyright notices.
- Record local modifications as an ordered patch set.
- Keep the vendored package under a private worker-owned namespace.
- Do not ship training, dataset, GUI, web, folder-scanning, or unrelated model-dispatch code.
- Keep model imports lazy outside real inference.
- Preserve upstream model architecture, checkpoint keys, chunking, overlap-add behavior, and core
  inference mathematics unless a measured compatibility issue justifies a recorded patch.

## 13. Maintenance workflow

For future changes:

1. Read `AGENTS.md` and this plan.
2. Inspect the current Git status, recent commits, and relevant implementation before editing.
3. Keep architecture and security boundaries intact.
4. Implement the smallest coherent change.
5. Update this plan only when a stable architectural decision, product invariant, or deferred item
   changes.
6. Keep implementation and directly related documentation in the same logical commit.

This document is no longer a phase-by-phase completion checklist. Completed historical phases and
unfinished manual test tasks have been removed to keep the active context small.

## 14. Deferred product work

These items are optional future refinements rather than incomplete MVP requirements:

- Code signing and distribution/update strategy.
- Independent GUI, runtime, model, and FFmpeg update policy.
- Runtime rollback after failed updates.
- Proxy and restricted-network support improvements.
- Measured installation peak-disk estimates.
- GPU memory profiling and adaptive CUDA OOM retry behavior.
- Exact source-rate alignment and sample-count enforcement.
- Explicit clipping or limiting policy.
- Metadata and cover-art copying.
- Optional mono-output policy.
- Crash recovery for abandoned job directories.

Keep detailed audio refinements in `docs/AUDIO_PIPELINE.md` and broader release operations in the
appropriate documentation rather than expanding this plan into a test matrix.

## 15. Build reference

Common repository commands:

```powershell
pnpm lint
pnpm typecheck
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
uv lock --project worker --check
pnpm tauri build --no-bundle
```

The standalone artifact is expected at:

```text
src-tauri/target/release/soufmer.exe
```

Manual functional and release validation is performed by the project owner and is not represented as
pending work in this document.

## 16. Definition of the implemented MVP

The implemented MVP provides:

- One movable Windows x64 executable with no required sidecars.
- No command prompt window during normal operation.
- Simplified Chinese default UI with English fallback.
- File and folder input, output-directory selection, and sequential processing.
- Compatibility and experimental source-sample-rate modes.
- Private first-run runtime initialization without Git or global Python changes.
- Reuse of a ready runtime on later launches.
- Real NVIDIA GPU model inference and accompaniment output.
- Initialization and processing progress, cancellation, completion, and localized error handling.
- Per-item temporary-file cleanup and safe partial-file publication.
- Embedded and accessible third-party notices.

## 17. Key decisions

- Use an online private runtime bootstrap because the full Python/CUDA/model environment is too large
  to bundle inside the portable executable.
- Keep every application-managed writable file beneath `%LOCALAPPDATA%\soufmer\`.
- Use compatibility mode by default because it reuses the exact model input for residual subtraction.
- Use SoXR precision 32 for all resampling; apply dither only once at final integer quantization.
- Treat the locked uv project as the only authority for the CUDA Python environment.
- Use the pinned SoXR-enabled BtbN FFmpeg build recorded in the runtime manifest and licensing files.
- Normalize canonical Windows paths only at validated external-process boundaries.
- Show structured initialization activity without pretending indeterminate work has a percentage.
- Use a Final Cut-inspired magenta theme with a darker accessible primary control color.
- Keep primary actions visible without normal page-level scrolling.

## 18. Documentation and references

Repository documentation:

- `README.md`: product and development overview.
- `docs/ARCHITECTURE.md`: component, IPC, runtime, state, and cancellation design.
- `docs/RUNTIME_MANIFEST.md`: manifest fields, hashes, and release preparation.
- `docs/AUDIO_PIPELINE.md`: compatibility/source-rate pipelines and deferred audio behavior.
- `docs/LICENSING.md`: project and third-party license obligations.
- `docs/SMOKE_TEST.md`: owner-managed manual validation notes.

Primary external references:

- Tauri 2: https://v2.tauri.app/
- shadcn/ui: https://ui.shadcn.com/
- Tailwind CSS: https://tailwindcss.com/
- uv: https://docs.astral.sh/uv/
- FFmpeg filters: https://ffmpeg.org/ffmpeg-filters.html
- FFmpeg resampler: https://ffmpeg.org/ffmpeg-resampler.html
- Music-Source-Separation-Training: https://github.com/ZFTurbo/Music-Source-Separation-Training
- KimberleyJSN MelBandRoformer: https://huggingface.co/KimberleyJSN/melbandroformer
