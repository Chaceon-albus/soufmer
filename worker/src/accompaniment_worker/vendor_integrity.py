"""Integrity checks for the packaged, minimal MSST inference closure."""

from __future__ import annotations

import hashlib
import json
from importlib.resources import files
from pathlib import Path
from typing import Any


def vendor_root() -> Path:
    packaged = Path(str(files("accompaniment_worker.vendor")))
    if (packaged / "source-manifest.json").is_file():
        return packaged
    return Path(__file__).resolve().parents[2] / "vendor"


def load_source_manifest(root: Path | None = None) -> dict[str, Any]:
    base = root if root is not None else vendor_root()
    with (base / "source-manifest.json").open("r", encoding="utf-8") as manifest_file:
        return json.load(manifest_file)


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_vendor_integrity(root: Path | None = None) -> None:
    """Fail on inventory, patch, or vendored-file drift without importing model code."""
    base = root if root is not None else vendor_root()
    manifest = load_source_manifest(base)
    if manifest.get("schemaVersion") != 1:
        raise ValueError("Unsupported vendor manifest schema")
    upstream = manifest.get("upstream")
    if not isinstance(upstream, dict) or len(upstream.get("commit", "")) != 40:
        raise ValueError("Vendor manifest has no immutable upstream commit")
    allowed = {"MSST_LICENSE", "UPSTREAM.md", "source-manifest.json", "model-manifest.json"}
    for entry in manifest.get("files", []):
        if _sha256(base / entry["path"]) != entry["vendoredSha256"]:
            raise ValueError(f"Vendored file hash mismatch: {entry['path']}")
        allowed.add(entry["path"])
    for patch in manifest.get("patches", []):
        if _sha256(base / patch["path"]) != patch["sha256"]:
            raise ValueError(f"Vendor patch hash mismatch: {patch['path']}")
        allowed.add(patch["path"])
    for path in base.rglob("*"):
        if not path.is_file() or "__pycache__" in path.parts or path.suffix == ".pyc":
            continue
        relative = path.relative_to(base).as_posix()
        if relative not in allowed:
            raise ValueError(f"Unrecorded vendored file: {relative}")
