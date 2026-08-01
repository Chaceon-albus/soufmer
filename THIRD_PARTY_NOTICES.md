# Third-Party Notices

This file records software and model components used by soufmer. Exact uv, vendored MSST, model,
and selected FFmpeg notices are embedded in the executable bootstrap and exposed by the
application's open-source attribution view. Resolved framework and package dependencies retain
their own licenses and remain governed by their corresponding locked dependency records.

## Tauri

The desktop application uses Tauri and its Rust and JavaScript dependencies. Tauri is distributed
under the Apache License 2.0 and MIT License. The release process must generate or otherwise retain
the exact notices required by the locked dependency graph.

Project: <https://tauri.app/>

## React and frontend dependencies

The user interface uses React, Vite, Tailwind CSS, shadcn/ui source components, Lucide, i18next,
and supporting packages. Their exact versions and license metadata are captured by
`pnpm-lock.yaml`. Release packaging must include the applicable notices for the resolved
dependency graph.

## uv

soufmer embeds uv `0.12.0` for `x86_64-pc-windows-msvc` from the immutable official release
archive. The archive SHA-256 is
`68200e25de594df92387186bbfb9d9df606ec1d87efaa0ae0c7f690970e53db6`; the extracted
`uv.exe` SHA-256 is `268cd62b99395eb53825795518e067e4b27ec4b445175df343824689f307c807`.
The exact MIT and Apache-2.0 license texts are included in the embedded bootstrap.

Project: <https://github.com/astral-sh/uv>

Source and checksum URLs are recorded in `src-tauri/bootstrap/runtime-manifest.json`.

## Music-Source-Separation-Training

soufmer includes a minimal, audited inference snapshot derived from
Music-Source-Separation-Training. The upstream MIT license and copyright notice must be preserved
with the vendored source, and all local modifications must remain identifiable.

Project: <https://github.com/ZFTurbo/Music-Source-Separation-Training>

Pinned source commit: `e247dfe4abc1f17c69dff719207fe045dc04413a`.

The copied-file inventory, source and vendored SHA-256 digests, upstream MIT text, and ordered local
patch record are stored under `worker/vendor/`.

## KimberleyJSN MelBandRoformer model

The runtime downloads a pinned revision of the KimberleyJSN MelBandRoformer checkpoint. The
release manifest and in-application license view record the exact revision, checkpoint SHA-256
digest, model attribution, and the repository's license declaration.

Project: <https://huggingface.co/KimberleyJSN/melbandroformer>

Pinned repository revision: `ac9b0614ab3cd7f77219e18ba494dfd93956c348`.

Checkpoint: `MelBandRoformer.ckpt`, 913,106,900 bytes.

Git-LFS SHA-256: `87201f4d31afb5bc79993230fc49446918425574db48c01c405e44f365c7559e`.

The machine-readable pinned source URLs and metadata are stored in
`worker/vendor/model-manifest.json`. The exact 21-byte README model card for this revision is
embedded as `src-tauri/bootstrap/licenses/MODEL_CARD.md`; it declares `license: mit` and has
SHA-256 `3e0e15fa0c5cc81675bd69af8eb469d128a725c1a7bfc71f03b7877b7b650567`.

## PyTorch, torchaudio, and Python runtime packages

The private worker environment contains PyTorch, torchaudio, and the complete locked worker
dependency set. Their exact versions, binary sources, and license files are determined by
`worker/uv.lock` and the selected CUDA runtime profile. Release notices must be generated from the
final locked environment.

Selected worker CUDA profile: official PyTorch CUDA 12.4 wheel index with version-matched
`torch==2.6.0` and `torchaudio==2.6.0`. The complete cross-platform resolution is captured by
`worker/uv.lock`; a clean Windows GPU synchronization and real inference remain release gates.

## FFmpeg and FFprobe

The runtime downloads the fixed Gyan.D FFmpeg `8.0.1` essentials Windows x64 ZIP linked by the
FFmpeg project. The archive is 106,259,850 bytes with SHA-256
`e2aaeaa0fdbc397d4794828086424d4aaa2102cef1fb6874f6ffd29c0b88b673`. This selected build is
GPL-3.0; the exact license file from the verified archive is included in the embedded bootstrap.

Project: <https://ffmpeg.org/>

The immutable archive URL, extraction layout, and license record are stored in
`src-tauri/bootstrap/runtime-manifest.json`.

## Microsoft Edge WebView2 Runtime

soufmer relies on the system Evergreen WebView2 Runtime supplied and maintained on supported
Windows installations. It is a system prerequisite and is not redistributed beside the portable
executable.

Product information: <https://developer.microsoft.com/microsoft-edge/webview2/>
