# Licensing

Original soufmer source is available under the repository's MIT License. Third-party components
retain their own licenses and attribution requirements; the project license does not replace them.

The release process preserves the upstream Music-Source-Separation-Training MIT notice for pinned
commit `e247dfe4abc1f17c69dff719207fe045dc04413a`; its copied-file inventory and ordered local patch
record are kept under `worker/vendor/`. The pinned KimberleyJSN repository revision is
`ac9b0614ab3cd7f77219e18ba494dfd93956c348`; its checkpoint identity is recorded in
`worker/vendor/model-manifest.json`. A release must confirm and archive that revision's repository
license declaration. The embedded uv 0.12.0 binary retains its MIT and Apache-2.0 texts. The
selected BtbN FFmpeg `n8.0.1-66-g27b8d1a017-20260228` build is LGPL-3.0 and its exact archive
`LICENSE.txt` is embedded. PyTorch, torchaudio, the private Python distribution, Tauri, and
the locked JavaScript/Rust/Python dependencies also require an exact resolved notice inventory.

`THIRD_PARTY_NOTICES.md` is the human-maintained top-level inventory. The embedded bootstrap must
contain complete license texts needed before the private runtime has been downloaded, and the
application must expose those notices through its license view before a public release.
