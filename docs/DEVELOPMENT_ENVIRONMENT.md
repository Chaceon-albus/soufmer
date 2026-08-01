# Local Development Environment

Observed on 2026-08-01. These values document one working development machine and are not
repository toolchain pins.

| Component | Observed version |
|---|---|
| Windows | 10.0.19044, x64 |
| Node.js | 24.18.1 |
| pnpm | 11.19.0 |
| Rust | 1.97.1 |
| Cargo | 1.97.1 |
| uv | 0.12.0 |
| Visual Studio | 18 Community |
| MSVC tools | 14.51.36231 |
| Windows SDK | 10.0.22621.0 and 10.0.26100.0 |
| Microsoft Edge WebView2 Runtime | 150.0.4078.105 |

The machine satisfies the Windows prerequisites needed to compile and run a Tauri 2 application.
Release testing still requires the clean-profile and missing-WebView2 cases described in
`docs/SMOKE_TEST.md`.
