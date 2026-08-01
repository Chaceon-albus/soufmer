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

- Provider: BtbN FFmpeg Builds, listed by https://ffmpeg.org/download.html as a Windows build provider.
- Release tag: `autobuild-2026-02-28-12-59`.
- Build identity: `n8.0.1-66-g27b8d1a017-20260228`.
- Archive: https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-02-28-12-59/ffmpeg-n8.0.1-66-g27b8d1a017-win64-lgpl-8.0.zip
- Published SHA-256 list: https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-02-28-12-59/checksums.sha256
- Archive SHA-256: `ef2b1179f226c7a953675623bff13e38ecd806a425f6f229e44660abdcd0c077`.
- The exact `LICENSE.txt` file included in that archive is copied to `FFMPEG_LGPL-3.0.txt`;
  its SHA-256 is `da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768`.

This LGPL build was selected because its reported configuration includes `--enable-libsoxr`; the
previous Gyan.D essentials ZIP was rejected after a functional SoXR test reported that the
requested resampling engine was unavailable. BtbN documents that its retained monthly releases
remain available for two years, so this artifact must be repinned before February 2028. BtbN also
states that its current Windows builds target Windows 10 22H2 and newer; earlier Windows 10
revisions are not guaranteed and are outside the validated compatibility range.

The FFmpeg archive is never embedded; the runtime installer must re-download, hash-verify, and
extract it under the private runtime root.

## KimberleyJSN MelBandRoformer model card

- Pinned revision: `ac9b0614ab3cd7f77219e18ba494dfd93956c348`.
- Exact raw card: https://huggingface.co/KimberleyJSN/melbandroformer/raw/ac9b0614ab3cd7f77219e18ba494dfd93956c348/README.md
- Immutable revision metadata: https://huggingface.co/api/models/KimberleyJSN/melbandroformer/revision/ac9b0614ab3cd7f77219e18ba494dfd93956c348
- Archived as `MODEL_CARD.md`, 21 bytes, SHA-256 `3e0e15fa0c5cc81675bd69af8eb469d128a725c1a7bfc71f03b7877b7b650567`.

The raw card declares `license: mit`; the Hugging Face revision metadata reports the same
`cardData.license` value.
