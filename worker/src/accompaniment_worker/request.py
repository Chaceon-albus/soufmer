"""Strict validation for one Rust-controlled worker request file."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from uuid import UUID

import soundfile as sf

from .errors import ErrorCode, WorkerFailure

_FIELDS = {"schemaVersion", "taskId", "inputPath", "outputVocalsPath", "checkpointPath", "configPath", "device", "batchSize", "overlap"}
_BATCH_SIZES = {1, 2, 4, 8, 16}
_OVERLAPS = {2, 4, 8}


@dataclass(frozen=True, slots=True)
class SeparationRequest:
    task_id: str
    input_path: Path
    output_vocals_path: Path
    checkpoint_path: Path
    config_path: Path
    device: str
    batch_size: int
    overlap: int


def _invalid(message: str) -> WorkerFailure:
    return WorkerFailure(ErrorCode.INVALID_REQUEST, message)


def _absolute_path(value: object, field: str) -> Path:
    if not isinstance(value, str):
        raise _invalid(f"{field} must be a string")
    path = Path(value)
    if not path.is_absolute():
        raise _invalid(f"{field} must be absolute")
    return path.resolve(strict=False)


def _within(path: Path, root: Path, field: str) -> None:
    try:
        path.relative_to(root)
    except ValueError as error:
        raise _invalid(f"{field} must remain within the assigned job directory") from error


def parse_request_file(request_path: Path) -> SeparationRequest:
    request_path = request_path.resolve(strict=False)
    try:
        with request_path.open("r", encoding="utf-8") as request_file:
            raw = json.load(request_file)
    except (OSError, json.JSONDecodeError) as error:
        raise _invalid("Request file is unreadable or invalid JSON") from error
    if not isinstance(raw, dict) or set(raw) != _FIELDS:
        raise _invalid("Request fields are invalid")
    if raw["schemaVersion"] != 1:
        raise _invalid("Unsupported request schema version")
    if not isinstance(raw["taskId"], str):
        raise _invalid("taskId must be a UUID")
    try:
        UUID(raw["taskId"])
    except ValueError as error:
        raise _invalid("taskId must be a UUID") from error

    request = SeparationRequest(
        task_id=raw["taskId"],
        input_path=_absolute_path(raw["inputPath"], "inputPath"),
        output_vocals_path=_absolute_path(raw["outputVocalsPath"], "outputVocalsPath"),
        checkpoint_path=_absolute_path(raw["checkpointPath"], "checkpointPath"),
        config_path=_absolute_path(raw["configPath"], "configPath"),
        device=raw["device"],
        batch_size=raw["batchSize"],
        overlap=raw["overlap"],
    )
    job_directory = request_path.parent
    _within(request_path, job_directory, "requestPath")
    _within(request.input_path, job_directory, "inputPath")
    _within(request.output_vocals_path, job_directory, "outputVocalsPath")
    if request.input_path == request.output_vocals_path:
        raise _invalid("inputPath and outputVocalsPath must differ")
    if request.output_vocals_path.suffix.lower() != ".wav":
        raise _invalid("outputVocalsPath must use the .wav extension")
    if request.device != "cuda:0" or type(request.batch_size) is not int or request.batch_size not in _BATCH_SIZES or type(request.overlap) is not int or request.overlap not in _OVERLAPS:
        raise _invalid("device, batchSize, or overlap is not an approved backend value")
    if not request.input_path.is_file() or not request.checkpoint_path.is_file() or not request.config_path.is_file():
        raise _invalid("Required request paths do not exist")
    return request


def validate_model_input(path: Path) -> None:
    """Validate model audio properties without importing model code."""
    try:
        info = sf.info(path)
    except RuntimeError as error:
        raise _invalid("inputPath cannot be inspected") from error
    if info.format != "WAV" or info.samplerate != 44_100 or info.channels != 2 or info.subtype != "FLOAT":
        raise _invalid("inputPath must be 44.1 kHz stereo Float32 WAV")
