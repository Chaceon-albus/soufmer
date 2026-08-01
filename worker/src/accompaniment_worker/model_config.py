"""Read the pinned Kimberley configuration without importing Torch or MSST modules."""

from __future__ import annotations

from pathlib import Path
from typing import Any
import hashlib

import yaml

from .errors import ErrorCode, WorkerFailure
from .vendor_integrity import load_source_manifest


class _ModelConfigLoader(yaml.SafeLoader):
    pass


def _construct_tuple(loader: yaml.Loader, node: yaml.Node) -> tuple[Any, ...]:
    return tuple(loader.construct_sequence(node))


_ModelConfigLoader.add_constructor("tag:yaml.org,2002:python/tuple", _construct_tuple)


def load_kimberley_config(path: Path) -> dict[str, Any]:
    try:
        content = path.read_bytes()
        expected_hash = next(
            entry["vendoredSha256"]
            for entry in load_source_manifest()["files"]
            if entry["path"] == "msst/configs/KimberleyJensen/config_vocals_mel_band_roformer_kj.yaml"
        )
        if hashlib.sha256(content).hexdigest() != expected_hash:
            raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model configuration does not match the pinned revision")
        raw = yaml.load(content.decode("utf-8"), Loader=_ModelConfigLoader)
    except (OSError, UnicodeDecodeError, StopIteration, yaml.YAMLError) as error:
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model configuration could not be loaded") from error
    if not isinstance(raw, dict):
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model configuration is invalid")
    audio, model, inference = raw.get("audio"), raw.get("model"), raw.get("inference")
    if not isinstance(audio, dict) or not isinstance(model, dict) or not isinstance(inference, dict):
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model configuration is invalid")
    if audio.get("sample_rate") != 44_100 or audio.get("num_channels") != 2 or model.get("stereo") is not True:
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model configuration is incompatible")
    return raw
