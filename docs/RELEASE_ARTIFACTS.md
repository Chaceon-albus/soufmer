# Release artifact preparation

This record documents the immutable bootstrap and first-run artifacts selected for the current
Windows x64 release profile. It is not a claim that a full portable release or GPU smoke test has
been completed.

## Embedded uv

- Version and target: uv `0.12.0`, `x86_64-pc-windows-msvc`.
- Official release archive: https://github.com/astral-sh/uv/releases/download/0.12.0/uv-x86_64-pc-windows-msvc.zip
- Published checksum: https://github.com/astral-sh/uv/releases/download/0.12.0/uv-x86_64-pc-windows-msvc.zip.sha256
- Archive size: 18,813,141 bytes.
- Archive SHA-256: `68200e25de594df92387186bbfb9d9df606ec1d87efaa0ae0c7f690970e53db6`.
- Extracted `uv.exe` SHA-256: `268cd62b99395eb53825795518e067e4b27ec4b445175df343824689f307c807`.
- Extracted `uv.exe` size: 47,514,624 bytes.
- License: dual MIT and Apache-2.0; exact texts are embedded under `src-tauri/bootstrap/licenses/`.

The archive checksum was fetched from the official release asset, then the archive was downloaded
to a temporary directory and hashed with Windows `Get-FileHash -Algorithm SHA256` before
extracting `uv.exe`.

## Downloaded FFmpeg runtime artifact

- Provider: [Gyan.D FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/), a Windows provider linked by [ffmpeg.org](https://ffmpeg.org/download.html).
- Versioned archive: https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.0.1-essentials_build.zip
- Published checksum: https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.0.1-essentials_build.zip.sha256
- Archive size: 106,259,850 bytes.
- Archive SHA-256: `e2aaeaa0fdbc397d4794828086424d4aaa2102cef1fb6874f6ffd29c0b88b673`.
- Archive root: `ffmpeg-8.0.1-essentials_build`.
- Binaries: `bin/ffmpeg.exe` and `bin/ffprobe.exe`.
- License classification: GPL-3.0. The exact `LICENSE` file in the verified ZIP is embedded as `FFMPEG_GPL-3.0.txt`.

The FFmpeg ZIP was downloaded to a temporary directory solely for independent SHA-256 verification
and ZIP layout/license inspection. It is not committed or embedded.

## Model and CUDA profile

- Model revision: `ac9b0614ab3cd7f77219e18ba494dfd93956c348`.
- Checkpoint: `MelBandRoformer.ckpt`, 913,106,900 bytes, SHA-256 `87201f4d31afb5bc79993230fc49446918425574db48c01c405e44f365c7559e`.
- Exact model card: [`README.md`](https://huggingface.co/KimberleyJSN/melbandroformer/raw/ac9b0614ab3cd7f77219e18ba494dfd93956c348/README.md), archived as `MODEL_CARD.md` (21 bytes, SHA-256 `3e0e15fa0c5cc81675bd69af8eb469d128a725c1a7bfc71f03b7877b7b650567`).
- License verification: the raw card declares `license: mit`; the [immutable Hugging Face revision API](https://huggingface.co/api/models/KimberleyJSN/melbandroformer/revision/ac9b0614ab3cd7f77219e18ba494dfd93956c348) reports `cardData.license` as `mit`.
- CUDA profile: official PyTorch CUDA 12.4 index, Torch `2.6.0+cu124`, torchaudio `2.6.0+cu124`.

The runtime manifest duplicates this release identity only where the Rust initializer needs it;
the worker remains authoritative for its detailed project and lock metadata.

## Estimates and remaining verification

The manifest uses conservative nonzero estimates: 5,000,000,000 bytes to download, 9,000,000,000
bytes installed, and 12,000,000,000 bytes minimum free space. These must be replaced with measured
clean-machine values before public release.

The build now generates and embeds a deterministic hash-manifested bootstrap archive, validates
its entries during extraction, and exposes the audited component attributions in the application.
`pnpm tauri build --no-bundle` produced an x64 Windows GUI executable, and a copy containing no
sibling files opened successfully from an otherwise empty development-machine directory. Still
required before public release: a clean locked CUDA sync using the embedded uv executable,
first-run FFmpeg/checkpoint download validation, worker CUDA self-test, full GPU audio smoke test,
and clean-profile or VM verification.
