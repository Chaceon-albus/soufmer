"""Load the checked-in worker configuration without importing inference dependencies."""

from __future__ import annotations

from dataclasses import dataclass
from importlib.resources import files
from pathlib import Path

import yaml

from .errors import ErrorCode, WorkerFailure


@dataclass(frozen=True, slots=True)
class WorkerConfig:
    schema_version: int
    model_id: str
    status: str
    inference_available: bool


def load_worker_config(path: Path | None = None) -> WorkerConfig:
    source = path if path is not None else files("accompaniment_worker.resources").joinpath("kimberley-melbandroformer.yaml")
    try:
        with open(source, "r", encoding="utf-8") as config_file:
            raw = yaml.safe_load(config_file)
    except (OSError, yaml.YAMLError) as error:
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Worker configuration could not be loaded") from error

    if not isinstance(raw, dict):
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Worker configuration is invalid")
    expected = {"schemaVersion", "modelId", "status", "inferenceAvailable"}
    if set(raw) != expected or raw["schemaVersion"] != 1 or raw["modelId"] != "KimberleyJSN/melbandroformer":
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Worker configuration is invalid")
    if raw["status"] not in {"NOT_CONFIGURED", "CONFIGURED"} or not isinstance(raw["inferenceAvailable"], bool):
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Worker configuration is invalid")
    if raw["status"] == "NOT_CONFIGURED" and raw["inferenceAvailable"]:
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Worker configuration is inconsistent")
    return WorkerConfig(raw["schemaVersion"], raw["modelId"], raw["status"], raw["inferenceAvailable"])
