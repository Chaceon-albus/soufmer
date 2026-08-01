# Smoke Test

Use generated tones or legally usable audio. Do not add copyrighted recordings to the repository.
Record the executable build identity, runtime version, GPU/driver, input properties, selected
options, result, and diagnostic ID for every failure.

## 2026-08-02 stabilization evidence

- Pre-fix reproduction used a generated one-second, 44.1 kHz stereo Float32 WAV under a path with
  spaces and Simplified Chinese characters. The worker request contained canonical drive paths in
  the form `\\?\C:\...` for the model input, vocals output, checkpoint, and configuration.
- The installed private worker rejected that request before inference with JSON Lines code
  `INVALID_REQUEST`, message `inputPath must remain within the assigned job directory`, and exit
  code `2`. Bounded stderr was empty. The Rust lifecycle mapper collapsed this stable worker code to
  `INFERENCE_FAILED`.
- After the process-boundary repair, the ignored Rust smoke
  `real_private_runtime_inference_publishes_local_output` passed in 19.65 seconds. It used the
  initialized private CUDA runtime, generated a Unicode/space-path fixture, produced and validated
  `vocals.wav`, built the compatibility residual, and published a validated Float32 WAV.
- `\\nas.local\WD\Media` was not accessible from the development machine on 2026-08-02.
  The real UNC inference smoke remains open; focused conversion coverage includes the exact
  `\\?\UNC\nas.local\WD\Media\...` regression form.

## Development checks

- Launch with `pnpm tauri dev`; verify the default locale is Simplified Chinese and no raw command
  text appears.
- Exercise keyboard focus, high-DPI scaling, file/folder selection, output selection, both mode
  cards, advanced options, and validation.
- At 100%, 150%, and 200% Windows display scaling, verify the native window fits the work area and
  all main-form and dialog content remains reachable by scrolling.
- At each scale, use only the keyboard to traverse every control; verify focus remains visible,
  radio cards and advanced-option checkboxes operate correctly, dialogs trap focus, and disabled
  actions remain visibly unavailable.
- Verify English fallback by switching the development locale.

## First-run runtime

- Use a Windows profile without global Python, uv, FFmpeg, or Git.
- Start the GUI, attach progress listeners, confirm the large download, and complete initialization.
- Verify all managed files stay under `%LOCALAPPDATA%\soufmer\`.
- Restart and verify a ready compatible runtime does not run `uv sync`.
- Cancel once during download and once during environment synchronization; retry safely.
- Start two copies and verify only one initializer mutates shared runtime state.

## Audio matrix

- 44.1 kHz stereo FLAC.
- 48 kHz stereo WAV or FLAC.
- 96 kHz stereo FLAC.
- MP3.
- M4A/AAC.
- Mono WAV.
- Paths containing spaces and Simplified Chinese characters, for example
  `C:\Users\Test\音乐 测试\输入.wav`.

For every compatible input, verify a 44.1 kHz stereo compatibility result. For 48 and 96 kHz
inputs, also verify experimental output uses the source sample rate. Inspect codec, sample format,
channel count, raw bit depth, duration, and absence of an abandoned `.partial` file.

## Batch behavior

- Process a folder recursively and non-recursively.
- Place the output directory inside the input tree and verify it is excluded.
- Exercise skip, overwrite, and auto-number conflict policies.
- Cancel during inference; confirm completed songs remain and the active process tree and temporary
  files are removed.
- Include one unsupported or corrupt item and verify safe later items continue.

## Real model and GPU

- Complete one short real KimberleyJSN MelBandRoformer inference on a supported NVIDIA GPU.
- Verify the checkpoint digest before loading and confirm the worker uses the intended CUDA Torch
  build.
- Exercise friendly `CUDA_NOT_AVAILABLE` behavior on a machine without usable CUDA.
- Exercise or simulate `CUDA_OUT_OF_MEMORY`; verify the stable code and diagnostics contain the
  technical detail while the normal dialog does not expose a traceback.

## Portable release

- Build with `pnpm tauri build --no-bundle`.
- Copy only `src-tauri\target\release\soufmer.exe` into an otherwise empty directory.
- Launch it from a read-only directory whose path contains spaces and Chinese characters.
- Move and rename the executable, launch again, and verify the private runtime is reused safely.
- Test on a clean supported Windows profile with Evergreen WebView2.
- Test the native recovery message on a system or controlled environment where WebView2 is absent.
- Remove the application by closing it, deleting the executable, and optionally deleting
  `%LOCALAPPDATA%\soufmer\` for the private runtime and settings.
