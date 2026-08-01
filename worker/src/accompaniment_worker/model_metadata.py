"""Pinned model release metadata, loaded without fetching model data."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from .errors import ErrorCode, WorkerFailure
from .vendor_integrity import vendor_root


@dataclass(frozen=True, slots=True)
class ModelMetadata:
    repository: str
    revision: str
    file_name: str
    download_url: str
    sha256: str
    size_bytes: int


def load_model_metadata(path: Path | None = None) -> ModelMetadata:
    source = path if path is not None else vendor_root() / "model-manifest.json"
    try:
        raw = json.loads(source.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model release metadata could not be loaded") from error
    model = raw.get("model") if isinstance(raw, dict) and raw.get("schemaVersion") == 1 else None
    card = raw.get("modelCard") if isinstance(raw, dict) else None
    if set(raw) != {"schemaVersion", "model", "modelCard"} or not isinstance(model, dict) or not isinstance(card, dict):
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model release metadata is invalid")
    required = {"repository", "revision", "fileName", "downloadUrl", "sha256", "sizeBytes", "source"}
    if set(model) != required or model["repository"] != "https://huggingface.co/KimberleyJSN/melbandroformer":
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model release metadata is invalid")
    if len(model["revision"]) != 40 or len(model["sha256"]) != 64 or not isinstance(model["sizeBytes"], int) or model["sizeBytes"] <= 0:
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model release metadata is invalid")
    expected_url = f"{model['repository']}/resolve/{model['revision']}/{model['fileName']}"
    if model["downloadUrl"] != expected_url:
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model release metadata is inconsistent")
    expected_card_url = f"{model['repository']}/raw/{model['revision']}/README.md"
    expected_api_url = f"https://huggingface.co/api/models/KimberleyJSN/melbandroformer/revision/{model['revision']}"
    if (
        set(card) != {"fileName", "rawUrl", "sha256", "sizeBytes", "license", "revisionApiUrl"}
        or card["fileName"] != "README.md"
        or card["rawUrl"] != expected_card_url
        or card["revisionApiUrl"] != expected_api_url
        or card["license"] != "mit"
        or not isinstance(card["sizeBytes"], int)
        or card["sizeBytes"] <= 0
        or not isinstance(card["sha256"], str)
        or len(card["sha256"]) != 64
        or not all(character in "0123456789abcdef" for character in card["sha256"])
    ):
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Model card metadata is invalid")
    return ModelMetadata(model["repository"], model["revision"], model["fileName"], model["downloadUrl"], model["sha256"], model["sizeBytes"])
