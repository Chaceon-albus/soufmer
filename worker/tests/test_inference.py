import json
from uuid import uuid4

import numpy as np
import pytest
import soundfile as sf

from accompaniment_worker.errors import ErrorCode, WorkerFailure
from accompaniment_worker.inference import Chunk, chunk_plan, extract_state_dict


def test_chunk_plan_uses_configured_overlap_step() -> None:
    assert chunk_plan(11, 8, 2) == [Chunk(0, 8), Chunk(4, 7), Chunk(8, 3)]


def test_state_dict_extraction_accepts_only_one_known_wrapper() -> None:
    state = {"layer.weight": object()}
    assert extract_state_dict({"state_dict": state}) is state
    with pytest.raises(WorkerFailure) as raised:
        extract_state_dict({"state_dict": state, "model_state_dict": state})
    assert raised.value.code is ErrorCode.MODEL_LOAD_FAILED


def test_state_dict_extraction_rejects_unstructured_checkpoint() -> None:
    with pytest.raises(WorkerFailure) as raised:
        extract_state_dict([])
    assert raised.value.code is ErrorCode.MODEL_LOAD_FAILED


def test_cli_forwards_bounded_chunk_progress_without_model_import(tmp_path, monkeypatch, capsys) -> None:
    from accompaniment_worker import cli
    from accompaniment_worker.vendor_integrity import vendor_root

    input_path = tmp_path / "input.wav"
    checkpoint_path = tmp_path / "MelBandRoformer.ckpt"
    output_path = tmp_path / "vocals.wav"
    sf.write(input_path, np.zeros((32, 2), dtype=np.float32), 44_100, format="WAV", subtype="FLOAT")
    checkpoint_path.write_bytes(b"unused fake checkpoint")
    request_path = tmp_path / "request.json"
    request_path.write_text(json.dumps({
        "schemaVersion": 1,
        "taskId": str(uuid4()),
        "inputPath": str(input_path),
        "outputVocalsPath": str(output_path),
        "checkpointPath": str(checkpoint_path),
        "configPath": str(vendor_root() / "msst/configs/KimberleyJensen/config_vocals_mel_band_roformer_kj.yaml"),
        "device": "cuda:0",
        "batchSize": 1,
        "overlap": 2,
    }), encoding="utf-8")

    def fake_separate(request, progress):
        progress(1, 3)
        progress(3, 3)

    monkeypatch.setattr(cli, "separate", fake_separate)
    assert cli._separate(request_path) == 0
    messages = [json.loads(line) for line in capsys.readouterr().out.splitlines()]
    assert [message["payload"] for message in messages if message["type"] == "progress"] == [{"current": 1, "total": 3}, {"current": 3, "total": 3}]
