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

- Provider: [BtbN FFmpeg Builds](https://github.com/BtbN/FFmpeg-Builds), a Windows provider linked by [ffmpeg.org](https://ffmpeg.org/download.html).
- Release tag and build: `autobuild-2026-02-28-12-59`, `n8.0.1-66-g27b8d1a017-20260228`.
- Versioned archive: https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-02-28-12-59/ffmpeg-n8.0.1-66-g27b8d1a017-win64-lgpl-8.0.zip
- Published checksum list: https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-02-28-12-59/checksums.sha256
- Archive size: 193,842,056 bytes.
- Archive SHA-256: `ef2b1179f226c7a953675623bff13e38ecd806a425f6f229e44660abdcd0c077`.
- Archive root: `ffmpeg-n8.0.1-66-g27b8d1a017-win64-lgpl-8.0`.
- Binaries: `bin/ffmpeg.exe` and `bin/ffprobe.exe`.
- License classification: LGPL-3.0. The exact `LICENSE.txt` in the verified ZIP is embedded as
  `FFMPEG_LGPL-3.0.txt`; its SHA-256 is
  `da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768`.

The FFmpeg ZIP was downloaded to a temporary directory solely for independent SHA-256 verification
and ZIP layout/license inspection. It is not committed or embedded. Its configuration reports
`--enable-libsoxr`, and the generated residual integration gate passed using SoXR precision 32.
The prior Gyan.D essentials ZIP was rejected because it lacked libsoxr.

BtbN retains monthly build releases for two years, so release maintenance must repin this artifact
before February 2028. BtbN currently targets Windows 10 22H2 and newer and does not guarantee
earlier Windows 10 revisions; portable release compatibility must be tested within that range.

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
