# MSST provenance

Source repository: https://github.com/ZFTurbo/Music-Source-Separation-Training

Pinned commit: `e247dfe4abc1f17c69dff719207fe045dc04413a`

Retrieved: 2026-07-30 from the GitHub commit archive. The archive SHA-256 is recorded in
`source-manifest.json`.

The product copies only the Mel-Band RoFormer architecture, its local attention module, the
Kimberley vocals configuration, and the upstream MIT license. It deliberately excludes upstream
folder enumeration, generic inference CLI, output naming, TTA, plotting, and training code.

Direct runtime imports after the recorded patch are Torch, packaging, einops, beartype,
rotary-embedding-torch, librosa, and optional `PoPE_pytorch`. The pinned Kimberley configuration
does not enable PoPE, so `PoPE_pytorch` is not a product dependency.

The audited CUDA profile is declared in `pyproject.toml` as the optional `cuda` extra: Torch 2.6.0
and torchaudio 2.6.0 from the explicit official CUDA 12.4 index. The fast test command does not
synchronize that large extra; production initialization does so with the exact locked,
non-editable uv command. Runtime readiness still requires an installed, hash-verified checkpoint
and a real GPU self-test.

Ordered local patches:

1. `0001-private-package-and-stderr-logging.patch` changes the local module import to the private
   worker namespace and redirects upstream informational output to logging. It does not alter
   model architecture or inference mathematics.

The selected source/configuration, checkpoint revision and hash, CUDA dependency profile, and
strict checkpoint-loading behavior are configured and recorded. A clean-machine locked sync and
real NVIDIA GPU inference remain manual release gates.
