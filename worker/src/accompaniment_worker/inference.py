"""Lazy, controlled single-file Mel-Band RoFormer inference."""

from __future__ import annotations

import os
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import soundfile as sf

from .errors import ErrorCode, WorkerFailure, map_exception
from .model_config import load_kimberley_config
from .model_metadata import load_model_metadata
from .request import SeparationRequest, validate_model_input

ProgressCallback = Callable[[int, int], None]


@dataclass(frozen=True, slots=True)
class Chunk:
    start: int
    length: int


def chunk_plan(length: int, chunk_size: int, overlap: int) -> list[Chunk]:
    """Return the pinned generic-MSST start positions without importing Torch."""
    if length <= 0 or chunk_size <= 0 or overlap <= 0:
        raise ValueError("Chunk parameters must be positive")
    step = chunk_size // overlap
    if step <= 0:
        raise ValueError("Chunk overlap is invalid")
    return [Chunk(start, min(chunk_size, length - start)) for start in range(0, length, step)]


def extract_state_dict(checkpoint: object) -> Mapping[str, Any]:
    """Select a known checkpoint wrapper deterministically; never tolerate partial weights."""
    if not isinstance(checkpoint, Mapping):
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Checkpoint does not contain a state dictionary")
    wrappers = [key for key in ("state_dict", "model_state_dict", "state") if isinstance(checkpoint.get(key), Mapping)]
    if wrappers:
        if len(wrappers) != 1:
            raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Checkpoint state dictionary is ambiguous")
        state_dict = checkpoint[wrappers[0]]
    else:
        state_dict = checkpoint
    if not state_dict or not all(isinstance(key, str) for key in state_dict):
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Checkpoint state dictionary is invalid")
    return state_dict


def validate_inference_entry(request: SeparationRequest) -> dict[str, Any]:
    """Validate controlled paths/configuration before importing Torch or the model."""
    validate_model_input(request.input_path)
    if not request.checkpoint_path.is_file():
        raise WorkerFailure(ErrorCode.INVALID_REQUEST, "checkpointPath does not exist")
    metadata = load_model_metadata()
    if request.checkpoint_path.name != metadata.file_name:
        raise WorkerFailure(ErrorCode.INVALID_REQUEST, "checkpointPath does not match the pinned model file")
    return load_kimberley_config(request.config_path)


def _load_runtime(config: dict[str, Any], checkpoint_path: Path, device_name: str) -> tuple[Any, Any, Any]:
    """Import the model only after all non-model validation has completed."""
    try:
        import torch
        from accompaniment_worker.vendor.msst.models.bs_roformer.mel_band_roformer import MelBandRoformer
    except ModuleNotFoundError as error:
        raise WorkerFailure(ErrorCode.RUNTIME_IMPORT_FAILED, "CUDA inference dependencies are unavailable") from error

    if not torch.cuda.is_available():
        raise WorkerFailure(ErrorCode.CUDA_NOT_AVAILABLE, "CUDA is unavailable")
    try:
        device = torch.device(device_name)
        # Force a real device allocation before loading several hundred megabytes of weights.
        torch.zeros(1, device=device).sum().item()
    except Exception as error:
        raise map_exception(error)

    try:
        model = MelBandRoformer(**dict(config["model"]))
        checkpoint = torch.load(checkpoint_path, weights_only=True, map_location="cpu")
        model.load_state_dict(extract_state_dict(checkpoint), strict=True)
        model.eval()
        model.to(device)
    except WorkerFailure:
        raise
    except Exception as error:
        raise WorkerFailure(ErrorCode.MODEL_LOAD_FAILED, "Pinned checkpoint could not be loaded") from error
    return torch, model, device


def _run_chunked_inference(
    torch: Any,
    model: Any,
    device: Any,
    mixture: np.ndarray,
    config: dict[str, Any],
    batch_size: int,
    overlap: int,
    progress: ProgressCallback | None,
) -> np.ndarray:
    """Adapt MSST's generic demix overlap-add path for one vocals-only input."""
    chunk_size = int(config["audio"]["chunk_size"])
    step = chunk_size // overlap
    border = chunk_size - step
    original_length = mixture.shape[1]
    mix = torch.tensor(mixture, dtype=torch.float32)
    if original_length > 2 * border and border > 0:
        mix = torch.nn.functional.pad(mix, (border, border), mode="reflect")
    padded_plans = chunk_plan(int(mix.shape[1]), chunk_size, overlap)
    fade_size = chunk_size // 10
    window = torch.ones(chunk_size, dtype=torch.float32)
    window[:fade_size] = torch.linspace(0, 1, fade_size)
    window[-fade_size:] = torch.linspace(1, 0, fade_size)
    result = torch.zeros((1, *mix.shape), dtype=torch.float32)
    counter = torch.zeros((1, *mix.shape), dtype=torch.float32)
    use_amp = bool(config.get("training", {}).get("use_amp", True))
    completed = 0

    with torch.inference_mode(), torch.amp.autocast(device_type="cuda", enabled=use_amp):
        for offset in range(0, len(padded_plans), batch_size):
            current = padded_plans[offset : offset + batch_size]
            parts = []
            for item in current:
                part = mix[:, item.start : item.start + chunk_size].to(device)
                pad_mode = "reflect" if item.length > chunk_size // 2 else "constant"
                parts.append(torch.nn.functional.pad(part, (0, chunk_size - item.length), mode=pad_mode, value=0))
            estimates = model(torch.stack(parts, dim=0))
            for index, item in enumerate(current):
                weighted_window = window.clone()
                if item.start == 0:
                    weighted_window[:fade_size] = 1
                elif item.start + step >= mix.shape[1]:
                    weighted_window[-fade_size:] = 1
                result[..., item.start : item.start + item.length] += estimates[index, ..., : item.length].detach().cpu() * weighted_window[: item.length]
                counter[..., item.start : item.start + item.length] += weighted_window[: item.length]
            completed += len(current)
            if progress is not None:
                progress(completed, len(padded_plans))

    vocals = result / counter.clamp_min(1e-8)
    output = vocals[0].cpu().numpy()
    np.nan_to_num(output, copy=False, nan=0.0, posinf=0.0, neginf=0.0)
    if original_length > 2 * border and border > 0:
        output = output[..., border:-border]
    if output.shape != mixture.shape or not np.isfinite(output).all():
        raise WorkerFailure(ErrorCode.INFERENCE_FAILED, "Model output is invalid")
    return output


def _partial_path(output_path: Path) -> Path:
    return output_path.with_name(f"{output_path.name}.partial")


def _write_output(output_path: Path, vocals: np.ndarray) -> None:
    partial = _partial_path(output_path)
    try:
        partial.unlink(missing_ok=True)
        sf.write(partial, vocals.T.astype(np.float32, copy=False), 44_100, format="WAV", subtype="FLOAT")
        info = sf.info(partial)
        if info.format != "WAV" or info.samplerate != 44_100 or info.channels != 2 or info.subtype != "FLOAT":
            raise WorkerFailure(ErrorCode.OUTPUT_WRITE_FAILED, "Vocals output validation failed")
        os.replace(partial, output_path)
    except WorkerFailure:
        partial.unlink(missing_ok=True)
        raise
    except Exception as error:
        partial.unlink(missing_ok=True)
        raise WorkerFailure(ErrorCode.OUTPUT_WRITE_FAILED, "Vocals output could not be written") from error


def separate(request: SeparationRequest, progress: ProgressCallback | None = None) -> None:
    """Process exactly one assigned WAV and atomically produce one vocals WAV."""
    partial = _partial_path(request.output_vocals_path)
    try:
        config = validate_inference_entry(request)
        mixture, sample_rate = sf.read(request.input_path, dtype="float32", always_2d=True)
        if sample_rate != 44_100 or mixture.shape[1] != 2:
            raise WorkerFailure(ErrorCode.INVALID_REQUEST, "inputPath changed after validation")
        torch, model, device = _load_runtime(config, request.checkpoint_path, request.device)
        vocals = _run_chunked_inference(torch, model, device, mixture.T, config, request.batch_size, request.overlap, progress)
        _write_output(request.output_vocals_path, vocals)
    except WorkerFailure:
        partial.unlink(missing_ok=True)
        raise
    except BaseException as error:
        partial.unlink(missing_ok=True)
        raise map_exception(error) from error
