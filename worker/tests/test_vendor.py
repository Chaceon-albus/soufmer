import ast
import json
import shutil
from pathlib import Path

import pytest

from accompaniment_worker.model_config import load_kimberley_config
from accompaniment_worker.vendor_integrity import load_source_manifest, vendor_root, verify_vendor_integrity


def test_vendor_manifest_and_patch_hashes_are_intact() -> None:
    verify_vendor_integrity()
    manifest = load_source_manifest()
    assert manifest["upstream"]["commit"] == "e247dfe4abc1f17c69dff719207fe045dc04413a"
    assert len(manifest["files"]) == 4


def test_cuda_extra_contains_every_vendored_model_import_dependency() -> None:
    pyproject = (vendor_root().parent / "pyproject.toml").read_text(encoding="utf-8")
    for dependency in ("beartype", "einops", "librosa", "numpy", "packaging", "rotary-embedding-torch", "torch", "torchaudio"):
        assert dependency in pyproject
    assert "https://download.pytorch.org/whl/cu124" in pyproject


def test_vendor_closure_contains_only_the_pinned_local_module_import() -> None:
    module = vendor_root() / "msst/models/bs_roformer/mel_band_roformer.py"
    imports = [node.module for node in ast.walk(ast.parse(module.read_text(encoding="utf-8"))) if isinstance(node, ast.ImportFrom)]
    assert "accompaniment_worker.vendor.msst.models.bs_roformer.attend" in imports
    assert not any(name and name.startswith("models.") for name in imports)


def test_vendor_drift_is_detected_without_importing_torch(tmp_path: Path) -> None:
    source = vendor_root()
    copied_vendor = tmp_path / "vendor"
    shutil.copytree(source, copied_vendor)
    changed_file = copied_vendor / "msst/models/bs_roformer/attend.py"
    changed_file.write_text(changed_file.read_text(encoding="utf-8") + "\n", encoding="utf-8")
    with pytest.raises(ValueError, match="Vendored file hash mismatch"):
        verify_vendor_integrity(copied_vendor)


def test_unrecorded_vendor_file_is_rejected_without_importing_torch(tmp_path: Path) -> None:
    source = vendor_root()
    copied_vendor = tmp_path / "vendor"
    shutil.copytree(source, copied_vendor)
    (copied_vendor / "unexpected.py").write_text("unexpected", encoding="utf-8")
    with pytest.raises(ValueError, match="Unrecorded vendored file"):
        verify_vendor_integrity(copied_vendor)


def test_pinned_kimberley_config_loads_without_model_import() -> None:
    config = load_kimberley_config(vendor_root() / "msst/configs/KimberleyJensen/config_vocals_mel_band_roformer_kj.yaml")
    assert config["audio"]["sample_rate"] == 44_100
    assert config["model"]["stereo"] is True


def test_model_card_record_is_pinned_to_the_model_revision() -> None:
    metadata = json.loads((vendor_root() / "model-manifest.json").read_text(encoding="utf-8"))
    card = metadata["modelCard"]
    revision = metadata["model"]["revision"]
    assert card["license"] == "mit"
    assert card["sizeBytes"] == 21
    assert card["sha256"] == "3e0e15fa0c5cc81675bd69af8eb469d128a725c1a7bfc71f03b7877b7b650567"
    assert card["rawUrl"].endswith(f"/{revision}/README.md")
    assert card["revisionApiUrl"].endswith(f"/revision/{revision}")
