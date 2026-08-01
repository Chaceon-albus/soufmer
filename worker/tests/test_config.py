from pathlib import Path
import sys

import pytest
import soundfile as sf
import numpy as np

from accompaniment_worker.config import load_worker_config
from accompaniment_worker.errors import ErrorCode, WorkerFailure
from accompaniment_worker.inference import validate_inference_entry
from accompaniment_worker.request import SeparationRequest
from accompaniment_worker.vendor_integrity import vendor_root


def test_checked_in_config_is_explicitly_not_configured() -> None:
    config = load_worker_config()
    assert config.model_id == "KimberleyJSN/melbandroformer"
    assert config.status == "NOT_CONFIGURED"
    assert not config.inference_available


def test_invalid_config_is_rejected(tmp_path: Path) -> None:
    config_path = tmp_path / "bad.yaml"
    config_path.write_text("status: CONFIGURED\n", encoding="utf-8")
    with pytest.raises(WorkerFailure) as raised:
        load_worker_config(config_path)
    assert raised.value.code is ErrorCode.MODEL_LOAD_FAILED


def test_inference_entry_validates_without_model_import(tmp_path: Path) -> None:
    config_path = vendor_root() / "msst/configs/KimberleyJensen/config_vocals_mel_band_roformer_kj.yaml"
    input_path = tmp_path / "input.wav"
    checkpoint_path = tmp_path / "MelBandRoformer.ckpt"
    sf.write(input_path, np.zeros((32, 2), dtype=np.float32), 44_100, format="WAV", subtype="FLOAT")
    checkpoint_path.write_bytes(b"not loaded by entry validation")
    request = SeparationRequest("task", input_path, tmp_path / "output.wav", checkpoint_path, config_path, "cuda:0", 1, 2)
    assert validate_inference_entry(request)["audio"]["sample_rate"] == 44_100
    assert "torch" not in sys.modules
    assert "accompaniment_worker.vendor.msst.models.bs_roformer.mel_band_roformer" not in sys.modules
