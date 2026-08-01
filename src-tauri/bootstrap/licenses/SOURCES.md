# Bootstrap artifact sources

## uv

- Release: `0.12.0`, target `x86_64-pc-windows-msvc`.
- Archive: https://github.com/astral-sh/uv/releases/download/0.12.0/uv-x86_64-pc-windows-msvc.zip
- Published SHA-256: https://github.com/astral-sh/uv/releases/download/0.12.0/uv-x86_64-pc-windows-msvc.zip.sha256
- Source licenses at the immutable release tag:
  - https://raw.githubusercontent.com/astral-sh/uv/0.12.0/LICENSE-MIT
  - https://raw.githubusercontent.com/astral-sh/uv/0.12.0/LICENSE-APACHE

The embedded `bin/uv.exe` was extracted from the archive only after its SHA-256 matched the
published checksum. It reports `uv 0.12.0`.

## FFmpeg

- Provider: Gyan.D FFmpeg Builds, listed by https://ffmpeg.org/download.html as a Windows build provider.
- Archive: https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.0.1-essentials_build.zip
- Published SHA-256: https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.0.1-essentials_build.zip.sha256
- The exact `LICENSE` file included in that archive is copied to `FFMPEG_GPL-3.0.txt`.

The FFmpeg archive is never embedded; the runtime installer must re-download, hash-verify, and
extract it under the private runtime root.

## KimberleyJSN MelBandRoformer model card

- Pinned revision: `ac9b0614ab3cd7f77219e18ba494dfd93956c348`.
- Exact raw card: https://huggingface.co/KimberleyJSN/melbandroformer/raw/ac9b0614ab3cd7f77219e18ba494dfd93956c348/README.md
- Immutable revision metadata: https://huggingface.co/api/models/KimberleyJSN/melbandroformer/revision/ac9b0614ab3cd7f77219e18ba494dfd93956c348
- Archived as `MODEL_CARD.md`, 21 bytes, SHA-256 `3e0e15fa0c5cc81675bd69af8eb469d128a725c1a7bfc71f03b7877b7b650567`.

The raw card declares `license: mit`; the Hugging Face revision metadata reports the same
`cardData.license` value.
