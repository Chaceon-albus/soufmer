"""JSON Lines protocol serialization for the Rust-owned process boundary."""

from __future__ import annotations

import json
import sys
from typing import Any, TextIO

from .errors import WorkerFailure

SCHEMA_VERSION = 1


def event(event_type: str, task_id: str, payload: dict[str, Any]) -> dict[str, Any]:
    return {
        "schemaVersion": SCHEMA_VERSION,
        "type": event_type,
        "taskId": task_id,
        "payload": payload,
    }


def error_event(task_id: str, failure: WorkerFailure) -> dict[str, Any]:
    return event(
        "error",
        task_id,
        {"code": failure.code.value, "recoverable": failure.recoverable, "message": failure.message},
    )


def write_event(payload: dict[str, Any], stream: TextIO | None = None) -> None:
    target = stream if stream is not None else sys.stdout
    target.write(json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n")
    target.flush()
