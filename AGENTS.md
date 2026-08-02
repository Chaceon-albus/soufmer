# AGENTS.md

## 1. Purpose

This repository contains a Windows desktop application for extracting music accompaniment with a Mel-Band RoFormer model.

The application is intended for non-technical users. It must ship as one movable executable, hide
command-line details, initialize its private runtime on first launch, process files sequentially,
and present clear Simplified Chinese UI text and progress feedback.

This file defines the repository-wide working rules for coding agents. Read it before modifying the repository. Use `docs/ARCHITECTURE.md` to understand the implemented system, durable design decisions, data flows, and runtime contracts. `AGENTS.md` is normative for agent behavior and repository constraints; `docs/ARCHITECTURE.md` is the durable technical description and must remain consistent with the implementation. Keep root-level guidance precise and broadly applicable; place genuinely subsystem-specific overrides in a nested `AGENTS.md` or `AGENTS.override.md` close to the affected code instead of duplicating or bloating this file.

## 2. Product scope

### MVP goals

- Windows 10 and Windows 11, x64.
- Tauri 2 desktop application.
- React + TypeScript + Vite frontend.
- Tailwind CSS and shadcn/ui for presentation components.
- Simplified Chinese as the default UI locale.
- English source code, identifiers, comments, logs, protocol fields, and developer documentation.
- File input and folder input.
- Output directory selection.
- Sequential processing only.
- Default 44.1 kHz compatibility residual mode.
- Experimental source-sample-rate residual mode.
- Two-level progress display for runtime initialization and audio processing.
- Private Python environment managed by `uv` embedded in the executable.
- Fixed FFmpeg/FFprobe build downloaded during runtime initialization.
- KimberleyJSN MelBandRoformer checkpoint downloaded during runtime initialization.
- A thin Python worker around the pinned and locally patched Music-Source-Separation-Training
  implementation.
- One portable `soufmer.exe` distribution artifact with no MSI/NSIS installer or required sibling
  file.
- All application-managed writable data under `%LOCALAPPDATA%\soufmer\`; user-selected input and
  output paths are the intentional exception.

### Explicit non-goals for the MVP

Do not implement these unless the user explicitly requests them or an approved task requires them:

- macOS or Linux support.
- Mobile support.
- Parallel song processing.
- Pause and resume of model inference.
- Multiple model selection.
- Model training.
- 5.1 or 7.1 audio.
- Video input.
- CUE-sheet handling.
- Network URL input.
- A generic terminal or console in the GUI.
- Arbitrary command execution from the frontend.
- Extensive test coverage, snapshot tests, or broad end-to-end test suites.
- An MSI, NSIS, or other installer.
- A fixed WebView2 runtime bundled beside the executable.

## 3. Language and localization rules

- Write all code, filenames, symbols, comments, logs, JSON fields, Rust errors, Python errors, and Markdown developer documentation in English.
- Do not place user-visible Chinese strings directly in React components.
- Use `i18next` and `react-i18next`.
- Store translations in:
  - `src/locales/en.json`
  - `src/locales/zh-CN.json`
- Use semantic English translation keys, for example:
  - `main.input.file`
  - `main.mode.compatibility.title`
  - `progress.stage.separating`
  - `error.cudaOutOfMemory`
- Use `zh-CN` as the default locale and English as the fallback locale.
- Keep the translation files structurally identical.
- User-facing errors must be translated from stable machine-readable error codes. Do not expose raw Python tracebacks or raw FFmpeg output in normal UI dialogs.

## 4. Toolchain policy

The development machine currently uses Windows 10, Rust 1.97.1, and pnpm 11.17.0. These tools may be updated with Scoop.

### Development tools

- Use the current stable Rust toolchain supported by Tauri 2.
- Do not pin an exact Rust patch release in `rust-toolchain.toml`.
- Prefer no `rust-toolchain.toml` for the MVP. If one becomes necessary, use `channel = "stable"`, not an exact version.
- Use pnpm 11 or newer.
- Use Node.js 22.12 or newer because current pnpm 11 and Vite requirements must both be satisfied.
- Do not hard-pin pnpm to an exact patch in `package.json`.
- It is acceptable to declare broad engine requirements such as:

```json
{
  "engines": {
    "node": ">=22.12",
    "pnpm": ">=11"
  }
}
```

- Commit `pnpm-lock.yaml` and `Cargo.lock` for reproducible application builds.
- Use normal compatible semver ranges in `package.json` and `Cargo.toml`; let lockfiles capture exact resolved versions.
- Upgrade dependencies deliberately. Do not mix dependency upgrades with unrelated feature work.

### End-user runtime

The end-user Python runtime must be reproducible even though the developer toolchain remains flexible.

- Commit `worker/uv.lock`.
- Keep the Python minor version fixed, preferably Python 3.11, while allowing uv to select the latest compatible patch.
- Pin Torch, torchaudio, Music-Source-Separation-Training code, model checkpoint revision, FFmpeg build, and download hashes in runtime manifests.
- Runtime installation must use `uv sync --locked --no-dev --extra cuda --no-editable
  --managed-python --no-python-downloads`.
- Declare every worker dependency in `worker/pyproject.toml` and commit `worker/uv.lock`. Do not
  repair a synchronized environment with `uv pip install`, repeated `uv add`, or a normal
  `--no-sync` workflow.
- Never install into the user's global Python environment.
- Never modify the user's `PATH`.

## 5. Required architecture

Use the following responsibility boundaries.

### React frontend

Responsible for:

- Input mode and path selection.
- Output directory selection.
- Processing mode selection.
- Advanced user options.
- Environment status display.
- Progress and completion dialogs.
- Localized validation and error presentation.

Not responsible for:

- Building command strings.
- Running Python, uv, FFmpeg, or FFprobe directly.
- Reading arbitrary filesystem paths without a backend command or approved Tauri plugin.
- Parsing raw terminal output.
- Deciding trusted download URLs or hashes.

### Rust/Tauri backend

Responsible for:

- Application state and task state machines.
- Runtime manifest loading and validation.
- Runtime directory management.
- Downloads, resume support, hashes, extraction, and atomic installation.
- Input enumeration and output path planning.
- FFprobe inspection.
- Child process creation and cancellation.
- Windows process-tree cleanup.
- Progress aggregation.
- Sequential job execution.
- Logs and diagnostics.
- Emitting typed progress events to the frontend.

### Python worker

Responsible for:

- Loading the pinned MelBandRoformer implementation and checkpoint.
- Running model inference against exactly one controlled input file per invocation.
- Producing a 44.1 kHz stereo Float32 vocals file.
- Emitting JSON Lines status messages.
- Returning stable error codes.

The Python worker must not:

- Enumerate the user's source folder.
- Choose output paths outside its assigned job directory.
- Download dependencies or models during an audio job.
- Display GUI elements.
- Perform final output encoding.

### FFmpeg/FFprobe

Responsible for:

- Audio inspection.
- Decode and controlled conversion to model input.
- SoXR resampling.
- Float-domain residual calculation for the MVP.
- Final encoding.

Invoke external processes with argument arrays. Never use shell-concatenated command strings.

## 6. Audio behavior

### Model input

Generate one controlled model input per song:

- Sample rate: 44,100 Hz.
- Channels: stereo.
- Sample format: Float32 PCM WAV.
- Resampler: SoXR.
- SoXR precision: 32.
- No dither while the signal remains floating point.

The logical filter must be equivalent to:

```text
aresample=resampler=soxr:osr=44100:precision=32
```

The exact FFmpeg argument construction belongs in Rust and must be covered by a focused test.

### Compatibility mode

This is the default mode.

- Reuse the exact model input WAV as the mixture side of the residual.
- Subtract the 44.1 kHz Float32 vocals output from that same model input.
- Do not decode and resample the source a second time for this mode.
- Produce a 44.1 kHz result.

### Source sample rate mode

This mode is experimental.

- Decode the original audio to Float32 at the source sample rate.
- Resample the 44.1 kHz vocals output to the source sample rate with SoXR precision 32.
- Subtract in Float32.
- Match the source sample rate.
- Match common source bit depths where the selected output codec supports them.
- Treat detailed time-alignment correction, resampler-delay compensation, and advanced clipping behavior as documented TODO items, not silent assumptions.

### Dither

- Do not dither Float32 intermediate files.
- Apply triangular dither only when a final Float32 signal is quantized to integer PCM and only once in the pipeline.

### Output and temporary files

- Process songs sequentially.
- Create a unique job directory for each song.
- Write final output to a `.partial` file in the final output directory.
- Validate the result with FFprobe.
- Rename the partial file to the final filename only after validation succeeds.
- Delete the current song's temporary files after success, failure cleanup, or cancellation.
- Preserve already completed songs when a batch is cancelled.

## 7. Runtime installation behavior

Use an online bootstrap architecture.

### Embedded in the standalone executable

- The compiled Tauri frontend and Rust application.
- One deterministic, hash-manifested bootstrap archive compiled into Rust with `include_bytes!`.
- A verified Windows x64 `uv.exe`.
- The Python worker source, `pyproject.toml`, `uv.lock`, selected MSST snapshot and ordered local
  patches, model config, and third-party license notices.
- A trusted root runtime manifest.

Do not use Tauri `bundle.resources`, an external binary sidecar, or a sibling directory for any
required bootstrap file. The release must still run when only `soufmer.exe` is copied to an empty
directory.

### Downloaded on first use

- Python managed by uv.
- Locked Python dependencies including the chosen Torch and torchaudio CUDA build.
- A fixed FFmpeg/FFprobe archive.
- The fixed KimberleyJSN MelBandRoformer checkpoint.

### Installation requirements

- No Git installation.
- No `git clone` on the user's computer.
- No Git/VCS MSST dependency in `pyproject.toml` or `uv.lock`; the audited MSST inference code is
  part of the embedded worker snapshot.
- No global Python, uv, FFmpeg, or PATH changes.
- Resolve `FOLDERID_LocalAppData` and use exactly `%LOCALAPPDATA%\soufmer\` as the one
  application-managed writable root.
- Never extract or write beside the executable, in the current working directory, or in `%TEMP%`.
- Download and extract only within versioned staging paths under the private root.
- Override `TEMP` and `TMP` for uv and worker child processes, set `UV_CACHE_DIR`, and route known
  Python/model caches into the private root. Do not use uv's `--no-cache` temporary fallback.
- Verify SHA-256 before extraction or activation.
- Reject archive path traversal, absolute paths, links/reparse points, duplicate normalized paths,
  and entries outside declared count/size limits.
- Install into versioned directories.
- Switch the active runtime only after a self-test passes.
- Keep a previously working runtime available until the new runtime is verified.
- Use an application-wide named mutex while mutating bootstrap or runtime state.
- First launch initializes after the GUI has attached progress listeners and the user confirms the
  large download. A later startup with a ready compatible runtime must not run `uv sync`.
- Execute the installed worker directly with the validated private Python; never use `uv run` for
  production audio jobs.

### WebView2 prerequisite

The one-file contract relies on the system Evergreen WebView2 Runtime supplied and updated by
supported Windows 10/11. Do not bundle a fixed WebView2 directory. When practical, check for it
before constructing the Tauri window and show a native localized recovery message if it is absent.

## 8. Frontend design rules

Use mature ecosystem components while keeping the project small.

### Required frontend stack

- React.
- TypeScript with strict mode.
- Vite.
- Tailwind CSS using the current Vite integration.
- shadcn/ui components.
- Lucide icons.
- `i18next` and `react-i18next`.
- `zod` for form and IPC payload validation where it materially improves safety.

### Avoid by default

- Redux.
- MobX.
- A frontend router for the MVP.
- TanStack Query.
- CSS-in-JS libraries.
- A second component framework.
- Hand-built primitive buttons, dialogs, progress bars, selects, or tooltips when shadcn/ui already provides them.

### State management

- Use a discriminated-union application state and `useReducer`.
- Do not represent lifecycle state with many independent booleans.
- Keep persisted user preferences separate from active task state.

Recommended top-level states:

```ts
type AppState =
  | { type: "idle" }
  | { type: "validating" }
  | { type: "initializing"; progress: InitializationProgress }
  | { type: "processing"; progress: BatchProgress }
  | { type: "cancelling"; lastProgress?: BatchProgress }
  | { type: "completed"; result: BatchResult }
  | { type: "failed"; error: AppError };
```

### Main screen

Keep the default screen limited to:

- File/folder segmented selector.
- Input path and browse button.
- Output directory and browse button.
- Radio-card processing mode selector.
- Collapsible advanced options.
- Environment status.
- Primary action button.

Advanced options may contain:

- Recursive folder scan.
- Preserve relative directory structure.
- Conflict policy: skip, overwrite, or auto-number.
- Output format.
- Option to generate both processing modes from one inference result.

### Progress dialog

Reuse one progress surface for initialization and processing.

It must contain:

- Task title.
- Overall progress bar.
- Overall count or stage indicator.
- Current item or current installation step.
- Current-task progress bar.
- Current stage text.
- Optional byte count and download speed during downloads.
- Elapsed time.
- Cancel button.

Use determinate progress only when the backend has measurable progress. Use an indeterminate state for commands that cannot report reliable percentages.

### Completion and error surfaces

Completion must show:

- Succeeded count.
- Failed count.
- Skipped count.
- Output directory.
- Open output directory action.
- Failed-item details when applicable.

Errors must show:

- Localized summary.
- Localized recovery action.
- Stable diagnostic code.
- A copy-diagnostics action.

Do not show raw command lines by default.

## 9. IPC and event rules

- Expose narrow Tauri commands such as `get_environment_status`, `initialize_environment`,
  `start_batch`, `cancel_active_task`, and `get_app_settings`.
- `start_batch` rejects an unready environment; it must not start an invisible dependency sync.
- Do not expose a generic `run_command` API.
- Validate all frontend payloads again in Rust.
- Commands return initial acknowledgements or final small results.
- Progress is pushed from Rust to React using typed Tauri events or channels.
- Use versioned event payloads if the protocol begins to evolve.
- Unsubscribe frontend event listeners on component unmount.

Recommended event envelope:

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

Recommended event types:

- `runtime://progress`
- `batch://progress`
- `batch://item-completed`
- `batch://completed`
- `task://failed`
- `task://cancelled`

## 10. Error handling and logging

- Define stable domain error codes in Rust.
- Map Python worker error codes and FFmpeg exit failures into the Rust error taxonomy.
- Log technical detail in English with structured fields.
- Store logs under the application's local data directory.
- Redact nothing necessary for local paths, but never include model download tokens, proxy credentials, or authorization headers.
- Keep the UI message short and actionable.
- Preserve stderr and worker traceback information in diagnostic logs, not normal dialogs.

Example domain codes:

- `ENV_NOT_INITIALIZED`
- `ENV_DOWNLOAD_FAILED`
- `ENV_HASH_MISMATCH`
- `PYTHON_SYNC_FAILED`
- `MODEL_DOWNLOAD_FAILED`
- `FFMPEG_NOT_AVAILABLE`
- `INPUT_UNSUPPORTED`
- `OUTPUT_NOT_WRITABLE`
- `CUDA_NOT_AVAILABLE`
- `CUDA_OUT_OF_MEMORY`
- `INFERENCE_FAILED`
- `POSTPROCESS_FAILED`
- `TASK_CANCELLED`

## 11. Security rules

- Keep external-process control in Rust.
- Do not grant broad Tauri shell permissions to the frontend.
- Use direct Rust process APIs for the hash-verified executables extracted from the embedded
  bootstrap or downloaded runtime.
- Never pass a user path through `cmd.exe /C`, PowerShell command text, or a shell-escaped string.
- Treat runtime manifests as trusted configuration and downloaded archives as untrusted until their hashes are verified.
- Treat the compiled bootstrap descriptor as trusted, but still verify its archive and per-entry
  hashes before activation.
- Canonicalize and validate important paths where appropriate.
- Prevent output directories from being recursively re-enumerated as input.
- Prevent source files from being overwritten unless the user explicitly selected overwrite behavior and the output path is distinct from the source path.

## 12. Dependency discipline

Before adding a dependency:

1. Confirm the standard library or an existing dependency cannot handle the task cleanly.
2. Prefer established projects with active maintenance.
3. Avoid overlapping libraries with the same responsibility.
4. Record the reason in the implementing commit and update `docs/ARCHITECTURE.md` when the dependency changes a durable architectural decision.

Preferred Rust dependencies may include:

- `serde` and `serde_json`.
- `tokio`.
- `thiserror`.
- `tracing` and `tracing-subscriber`.
- `reqwest` for controlled downloads.
- `sha2` for verification.
- `zip` or another format-specific extraction crate.
- `uuid`.
- `directories`.
- `walkdir`.
- `windows` or `windows-sys` only where Windows Job Objects or process flags are required.

Do not add a database for the MVP. JSON settings and versioned manifests are sufficient.

## 13. Testing policy

The user explicitly does not want a large test suite. Test only logic where regressions would be costly or difficult to detect manually.

### Required automated tests

#### Rust

- Input enumeration and output path planning.
- Filename conflict policy.
- Runtime manifest parsing and hash verification behavior.
- Progress aggregation math.
- Cancellation state transition.

#### Python

- JSON Lines protocol serialization.
- Worker configuration loading.
- A lightweight inference-entry validation that does not load the full model.

#### Audio integration

Use generated short audio fixtures, not copyrighted music.

- Zero-vocals identity: residual output matches the mixture within an appropriate Float32 tolerance.
- Full-vocals cancellation: using the mixture as the vocals input produces near-silence in compatibility mode.
- Output inspection: expected sample rate, channel count, and codec properties.

#### Frontend

- One reducer/state-machine test covering the normal lifecycle.
- One validation test for the main form.

### Tests not required for MVP

- Snapshot tests.
- Tests for every shadcn component.
- Pixel-perfect visual regression.
- Full model inference in normal CI.
- Large browser E2E suites.
- Exhaustive error-branch unit tests.

### Manual smoke tests

Maintain a short `docs/SMOKE_TEST.md` checklist for:

- 44.1 kHz stereo FLAC.
- 48 kHz stereo WAV or FLAC.
- 96 kHz stereo FLAC.
- MP3.
- M4A/AAC.
- Mono input.
- Chinese and special-character paths.
- Cancellation.
- Existing output conflict.
- First-run initialization.
- A real NVIDIA GPU inference run.

## 14. Quality gates

Before declaring a completed change or release checkpoint ready, run the applicable commands.

Frontend:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Rust:

```powershell
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Python worker:

```powershell
uv lock --project worker --check
uv run --project worker --locked pytest worker/tests

# Run separately when validating the installed CUDA/model path.
uv run --project worker --locked --extra cuda python -m accompaniment_worker self-test
```

Desktop smoke build:

```powershell
pnpm tauri build --no-bundle
```

For a release checkpoint, copy only `src-tauri/target/release/soufmer.exe` into an otherwise empty
directory and test it from a clean Windows profile. It must not require a sibling resource, DLL, or
writable executable directory. Normal Windows system DLLs and the system Evergreen WebView2
runtime are prerequisites, not shipped sidecars.

Do not suppress warnings merely to make a quality gate pass. Fix the root cause or document a narrowly scoped exception.

## 15. Repository and code organization

Target structure:

```text
.
├─ AGENTS.md
├─ LICENSE
├─ THIRD_PARTY_NOTICES.md
├─ package.json
├─ pnpm-lock.yaml
├─ vite.config.ts
├─ src/
│  ├─ app/
│  ├─ components/
│  │  ├─ ui/
│  │  └─ feature/
│  ├─ hooks/
│  ├─ lib/
│  ├─ locales/
│  ├─ types/
│  └─ main.tsx
├─ src-tauri/
│  ├─ Cargo.toml
│  ├─ Cargo.lock
│  ├─ build.rs
│  ├─ capabilities/
│  ├─ bootstrap/
│  │  ├─ bin/uv.exe
│  │  ├─ runtime-manifest.json
│  │  └─ licenses/
│  └─ src/
│     ├─ commands/
│     ├─ domain/
│     ├─ runtime/
│     ├─ jobs/
│     ├─ audio/
│     ├─ process/
│     ├─ progress/
│     └─ lib.rs
├─ worker/
│  ├─ pyproject.toml
│  ├─ uv.lock
│  ├─ src/accompaniment_worker/
│  ├─ vendor/msst/
│  ├─ vendor/patches/
│  ├─ vendor/source-manifest.json
│  ├─ vendor/MSST_LICENSE
│  ├─ vendor/UPSTREAM.md
│  └─ tests/
└─ docs/
   ├─ ARCHITECTURE.md
   ├─ RUNTIME_MANIFEST.md
   └─ SMOKE_TEST.md
```

Do not create layers or abstractions that are unused. Prefer cohesive modules with explicit data flow.

## 16. Licensing rules

- The application repository may use the MIT License.
- Preserve the original MIT copyright and license notices for modified or vendored Music-Source-Separation-Training code.
- Include model attribution and the model repository's MIT declaration in `THIRD_PARTY_NOTICES.md` and the application's license view.
- Include the uv license and attribution for the embedded `uv.exe`.
- Include the exact FFmpeg build license files and identify whether the selected build is LGPL or GPL.
- Do not assume the application MIT License replaces third-party licenses.
- Keep third-party code modifications identifiable, either through a patch record, a vendor README, or clear source comments.

## 17. Agent workflow

### Planning and architecture records

- Use Codex `/plan` mode for non-trivial work that benefits from explicit sequencing, risk analysis, or checkpoints. The plan is session-local working state; do not recreate a persistent repository-wide implementation-plan file unless the user explicitly asks for one.
- Read `docs/ARCHITECTURE.md` before changing component boundaries, runtime behavior, IPC or worker protocols, storage layout, security boundaries, audio processing, build/distribution behavior, or other documented invariants.
- Update `docs/ARCHITECTURE.md` in the same logical commit whenever an architectural change alters responsibilities, data flow, public/internal protocols, runtime or storage contracts, security assumptions, deployment, or a durable technical decision.
- For other major changes, use judgment: update the architecture document when it would otherwise become incomplete or misleading for the next maintainer. Routine bug fixes, local refactors, tests, copy changes, and minor styling changes do not require an architecture update unless they change documented behavior or invariants.
- Keep transient task steps, experiments, and unaccepted alternatives out of `docs/ARCHITECTURE.md`. Record only the implemented state and durable decisions.

### Task workflow

1. Read this file, applicable nested instructions, and the relevant parts of `docs/ARCHITECTURE.md` before editing.
2. For a non-trivial task, use `/plan` mode to define the smallest coherent results, acceptance criteria, dependencies, and validation sequence.
3. Inspect the implementation before making architectural changes.
4. Establish the repository state with `git status --short`, `git diff --stat`, `git diff`, and `git log --oneline --decorate -n 15`. When untracked files exist, also run `git ls-files --others --exclude-standard`.
5. Treat pre-existing tracked and untracked changes as user work until their purpose is understood. Never discard them merely to obtain a clean worktree.
6. Implement one independently reviewable unit at a time. Add only focused tests justified by Section 13 and run focused checks while iterating.
7. Update architecture documentation, manifests, lockfiles, translations, and other durable documentation when the completed unit changes their recorded state.
8. Commit each completed unit under the policy below before switching concerns.
9. Run all applicable Section 14 quality gates before declaring a completed change or release checkpoint ready.
10. Do not stop after planning unless the user requested planning only or a destructive or materially ambiguous decision requires confirmation. Do not silently expand scope.

### Commit policy

A commit unit is an independently reviewable, internally consistent, and independently testable logical change. A large task may contain several commit units and normally should produce several commits when it spans distinct concerns.

Typical units are one behavior or module with focused tests; one narrow bug fix; one behavior-preserving refactor; one dependency/runtime/vendor update with required lockfiles, manifests, hashes, licenses, and validation; or one focused build, documentation, localization, or test-infrastructure change.

Elapsed time, file count, individual file edits, a broad task boundary, and catch-all labels such as "misc changes" are not sufficient boundaries. Commit only when one coherent unit is complete, relevant checks pass or a missing check is documented, required supporting artifacts are included, no known incomplete behavior is presented as complete, and the staged diff contains only that unit.

Commit before switching to an unrelated subsystem or concern, beginning a separate dependency/runtime/vendor change, pausing with completed work, or declaring the requested task complete. Keep the worktree bounded to one active incomplete concern whenever practical; do not wait for the end of a large task.

### Staging and commit procedure

Before every commit:

1. Run `git status --short` and inspect all changed and untracked paths.
2. Inspect the intended path diffs.
3. Stage with explicit pathspecs. Do not use `git add .` or `git add -A`.
4. Run `git diff --cached --check`.
5. Review the complete staged patch with `git diff --cached`; exclude unrelated user work.
6. Run or confirm the relevant focused validation.
7. Commit with a concise English subject describing the completed result.
8. Run `git status --short` and `git log -1 --oneline` immediately afterward.

Additional rules:

- Keep behavior and its tests together unless the tests are an independent infrastructure change.
- Update `docs/ARCHITECTURE.md` under the architecture-maintenance rules above; do not document an intended architecture as implemented before the code and required validation support it.
- Commit required reproducible lockfiles and manifests, but never caches, temporary files, local environments, build outputs, secrets, tokens, or accidental downloads.
- Avoid WIP, empty, and knowingly broken commits. Leave incomplete work uncommitted and report it.
- Follow repository history. If no convention exists, use a concise Conventional Commit-style subject such as `fix(worker): reject invalid model metadata`.

### Recovery and Git safety

When resuming with existing changes, classify them by purpose and provenance. Validate and commit complete coherent units; preserve incomplete work uncommitted; preserve uncertain changes and report them instead of guessing. If unrelated work cannot be isolated safely, ask before committing.

Do not use `git reset`, `git clean`, `git restore`, `git checkout --`, `git stash`, amend, rebase, squash, or other history-rewriting or destructive operations unless the user explicitly authorizes the exact action. Do not bypass hooks or suppress warnings merely to make a commit pass.

Never push, force-push, create a pull request, publish a release, create a remote tag, or modify remotes without a separate explicit user request.

### Subagent coordination

Use subagents only for bounded work that materially improves speed or review quality. Medium reasoning is the default; use higher reasoning for complex tracing, security review, or difficult edge cases.

Give each subagent a narrow objective, allowed files, constraints, acceptance criteria, validation commands, and a report format covering files, test results, assumptions, risks, and unresolved issues. Parallelize read-only or independent work; serialize writes that share files, interfaces, manifests, or architecture.

The main agent reviews actual diffs and evidence and alone stages and commits. Subagents must not stage, commit, rewrite history, discard work, stash, push, tag, or modify remotes.

### Completion report

On completion or a genuine blocker, report completed work, remaining work, commit hashes and subjects, validation results, intentionally uncommitted files with reasons, and material assumptions or risks.

When requirements are ambiguous, prefer the simplest behavior consistent with this file and `docs/ARCHITECTURE.md`. Mark deferred audio-quality refinements with explicit TODO references instead of inventing unverified signal-processing behavior.

## 18. Official references

Use current official documentation when setup syntax has changed:

- Tauri 2 project setup: https://v2.tauri.app/start/create-project/
- Tauri Vite integration: https://v2.tauri.app/start/frontend/vite/
- Tauri frontend/Rust commands: https://v2.tauri.app/develop/calling-rust/
- Tauri Rust/frontend events and channels: https://v2.tauri.app/develop/calling-frontend/
- Tauri dialog plugin: https://v2.tauri.app/plugin/dialog/
- Tauri build and `--no-bundle`: https://v2.tauri.app/distribute/
- Tauri resource behavior: https://v2.tauri.app/develop/resources/
- Tauri Windows WebView2 guidance: https://v2.tauri.app/distribute/windows-installer/#webview2-installation-options
- shadcn/ui Vite setup: https://ui.shadcn.com/docs/installation/vite
- Tailwind CSS Vite setup: https://tailwindcss.com/docs/installation/using-vite
- uv PyTorch integration: https://docs.astral.sh/uv/guides/integration/pytorch/
- uv storage locations and overrides: https://docs.astral.sh/uv/reference/storage/
- Music-Source-Separation-Training: https://github.com/ZFTurbo/Music-Source-Separation-Training
- KimberleyJSN MelBandRoformer: https://huggingface.co/KimberleyJSN/melbandroformer
- OpenAI Codex AGENTS.md guidance: https://developers.openai.com/codex/agent-configuration/agents-md
- OpenAI Codex subagents guidance: https://developers.openai.com/codex/agent-configuration/subagents
- OpenAI prompting guidance: https://developers.openai.com/codex/prompting
- AGENTS.md open format: https://agents.md/
