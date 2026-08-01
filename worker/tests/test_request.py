import json
from pathlib import Path
from uuid import uuid4

import pytest

from accompaniment_worker.errors import ErrorCode, WorkerFailure
from accompaniment_worker.request import parse_request_file


def _request(job: Path) -> dict[str, object]:
    (job / "input.wav").write_bytes(b"placeholder")
    (job / "model.ckpt").write_bytes(b"checkpoint")
    (job / "config.yaml").write_text("schemaVersion: 1\nmodelId: KimberleyJSN/melbandroformer\nstatus: NOT_CONFIGURED\ninferenceAvailable: false\n", encoding="utf-8")
    return {"schemaVersion": 1, "taskId": str(uuid4()), "inputPath": str((job / "input.wav").resolve()), "outputVocalsPath": str((job / "vocals.wav").resolve()), "checkpointPath": str((job / "model.ckpt").resolve()), "configPath": str((job / "config.yaml").resolve()), "device": "cuda:0", "batchSize": 1, "overlap": 2}


def test_request_validation_accepts_one_controlled_job(tmp_path: Path) -> None:
    request_path = tmp_path / "request.json"
    request_path.write_text(json.dumps(_request(tmp_path)), encoding="utf-8")
    request = parse_request_file(request_path)
    assert request.input_path.parent == tmp_path.resolve()


def test_request_validation_rejects_output_outside_job(tmp_path: Path) -> None:
    request_path = tmp_path / "request.json"
    request = _request(tmp_path)
    request["outputVocalsPath"] = str((tmp_path.parent / "outside.wav").resolve())
    request_path.write_text(json.dumps(request), encoding="utf-8")
    with pytest.raises(WorkerFailure) as raised:
        parse_request_file(request_path)
    assert raised.value.code is ErrorCode.INVALID_REQUEST
