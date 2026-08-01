import json
from pathlib import Path

import pytest

from accompaniment_worker.errors import ErrorCode, WorkerFailure
from accompaniment_worker.model_metadata import load_model_metadata
from accompaniment_worker.vendor_integrity import vendor_root


def test_model_metadata_pins_lfs_object_without_fetching_it() -> None:
    metadata = load_model_metadata()
    assert metadata.revision == "ac9b0614ab3cd7f77219e18ba494dfd93956c348"
    assert metadata.file_name == "MelBandRoformer.ckpt"
    assert metadata.sha256 == "87201f4d31afb5bc79993230fc49446918425574db48c01c405e44f365c7559e"
    assert metadata.size_bytes == 913_106_900


def test_model_metadata_rejects_non_revision_specific_url(tmp_path: Path) -> None:
    manifest = json.loads((vendor_root() / "model-manifest.json").read_text(encoding="utf-8"))
    manifest["model"]["downloadUrl"] = "https://huggingface.co/KimberleyJSN/melbandroformer/resolve/main/MelBandRoformer.ckpt"
    path = tmp_path / "model-manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    with pytest.raises(WorkerFailure) as raised:
        load_model_metadata(path)
    assert raised.value.code is ErrorCode.MODEL_LOAD_FAILED
