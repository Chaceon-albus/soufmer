import io
import json

from accompaniment_worker.errors import ErrorCode, WorkerFailure
from accompaniment_worker.protocol import error_event, event, write_event


def test_json_lines_event_serialization() -> None:
    stream = io.StringIO()
    write_event(event("progress", "task-1", {"current": 1, "total": 2}), stream)
    assert json.loads(stream.getvalue()) == {"schemaVersion": 1, "type": "progress", "taskId": "task-1", "payload": {"current": 1, "total": 2}}


def test_error_event_uses_stable_code() -> None:
    output = error_event("task-1", WorkerFailure(ErrorCode.CUDA_OUT_OF_MEMORY, "CUDA out of memory", True))
    assert output["payload"]["code"] == "CUDA_OUT_OF_MEMORY"
