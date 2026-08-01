# soufmer

soufmer is a Windows desktop application for extracting music accompaniment with a
Mel-Band RoFormer model. It is designed for non-technical users and targets one movable Tauri
executable with a private, first-run runtime under `%LOCALAPPDATA%\soufmer\`.

The application and its private-runtime bootstrap are implemented. The embedded uv executable,
FFmpeg archive, CUDA dependency profile, model revision/checkpoint, and vendored MSST snapshot are
pinned with verified identities and hashes. A public release still requires the manual
clean-profile, CUDA/GPU, and one-file smoke checks documented in `docs/SMOKE_TEST.md`.

## Supported development platform

- Windows 10 or 11, x64
- Node.js 22.12 or newer
- pnpm 11 or newer
- Current stable Rust supported by Tauri 2
- Microsoft C++ build tools, a Windows SDK, and Evergreen WebView2 Runtime
- uv for worker development

Exact application dependencies are captured by `pnpm-lock.yaml`, `src-tauri/Cargo.lock`, and
`worker/uv.lock`. Developer tool patch versions are not repository-pinned.

## Development

```powershell
pnpm install
pnpm tauri dev
```

Run focused quality gates with:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build

cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml

uv lock --project worker --check
uv run --project worker --locked pytest worker/tests
```

The normal fast tests do not download or load the model.

## Runtime release preparation

The audited runtime identities and SHA-256 digests are recorded in
`docs/RUNTIME_MANIFEST.md`, `docs/RELEASE_ARTIFACTS.md`, and `THIRD_PARTY_NOTICES.md`. A release is
valid only after the clean locked CUDA sync, real GPU inference, raw one-file build, and
clean-profile smoke checks in `docs/SMOKE_TEST.md`.

## License

Original project code is licensed under the MIT License. Third-party software and model artifacts
retain their own licenses; see `THIRD_PARTY_NOTICES.md` and `docs/LICENSING.md`.
