"""Stable worker error codes and exit-code mapping."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


class ErrorCode(str, Enum):
    INVALID_REQUEST = "INVALID_REQUEST"
    RUNTIME_IMPORT_FAILED = "RUNTIME_IMPORT_FAILED"
    MODEL_LOAD_FAILED = "MODEL_LOAD_FAILED"
    MODEL_NOT_CONFIGURED = "MODEL_NOT_CONFIGURED"
    CUDA_NOT_AVAILABLE = "CUDA_NOT_AVAILABLE"
    CUDA_OUT_OF_MEMORY = "CUDA_OUT_OF_MEMORY"
    INFERENCE_FAILED = "INFERENCE_FAILED"
    OUTPUT_WRITE_FAILED = "OUTPUT_WRITE_FAILED"
    TASK_CANCELLED = "TASK_CANCELLED"


EXIT_CODES = {
    ErrorCode.INVALID_REQUEST: 2,
    ErrorCode.RUNTIME_IMPORT_FAILED: 10,
    ErrorCode.MODEL_LOAD_FAILED: 11,
    ErrorCode.MODEL_NOT_CONFIGURED: 11,
    ErrorCode.CUDA_NOT_AVAILABLE: 12,
    ErrorCode.CUDA_OUT_OF_MEMORY: 13,
    ErrorCode.INFERENCE_FAILED: 14,
    ErrorCode.OUTPUT_WRITE_FAILED: 15,
    ErrorCode.TASK_CANCELLED: 130,
}


@dataclass(slots=True)
class WorkerFailure(Exception):
    code: ErrorCode
    message: str
    recoverable: bool = False

    @property
    def exit_code(self) -> int:
        return EXIT_CODES[self.code]


def map_exception(error: BaseException) -> WorkerFailure:
    """Map known runtime failures without importing torch or model code."""
    if isinstance(error, WorkerFailure):
        return error
    if isinstance(error, (KeyboardInterrupt, InterruptedError)):
        return WorkerFailure(ErrorCode.TASK_CANCELLED, "Task cancelled", True)
    if isinstance(error, ModuleNotFoundError):
        return WorkerFailure(ErrorCode.RUNTIME_IMPORT_FAILED, "Required runtime module is unavailable")

    text = str(error).lower()
    if "out of memory" in text and ("cuda" in text or "torch" in text):
        return WorkerFailure(ErrorCode.CUDA_OUT_OF_MEMORY, "CUDA out of memory", True)
    if "cuda" in text and ("not available" in text or "not compiled" in text):
        return WorkerFailure(ErrorCode.CUDA_NOT_AVAILABLE, "CUDA is unavailable")
    return WorkerFailure(ErrorCode.INFERENCE_FAILED, "Inference failed")
