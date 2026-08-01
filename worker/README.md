# Worker foundation

This package owns the controlled, one-file Python worker protocol. It vendors a pinned minimal
Mel-Band RoFormer import closure and release metadata, but does not claim a usable runtime until
the Rust installer has installed the hash-verified checkpoint and locked CUDA environment.

`self-test` without arguments reports `NOT_CONFIGURED`. A real runtime check must pass both
`--checkpoint <MelBandRoformer.ckpt>` and the exact vendored configuration at
`vendor/msst/configs/KimberleyJensen/config_vocals_mel_band_roformer_kj.yaml`; the worker rejects
other configuration content by its pinned SHA-256. It loads the checkpoint with `weights_only=True`,
validates its state dictionary strictly, and runs a tiny CUDA operation before reporting `READY`.
Fast tests never invoke that path.

The production launcher must invoke `python -m accompaniment_worker separate --request <file>`
with direct process arguments. Worker standard output is JSON Lines only. The lifecycle is
`ready`, `stage` (`loadingModel`, then `separating`), `progress`, and `completed`; failures emit
an `error` record with a machine-readable code and a nonzero exit status.
