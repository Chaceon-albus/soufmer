"""CLI surface used only by the Rust process launcher."""

from __future__ import annotations

import argparse
from pathlib import Path

from .config import load_worker_config
from .errors import ErrorCode, WorkerFailure, map_exception
from .inference import _load_runtime, separate
from .model_config import load_kimberley_config
from .model_metadata import load_model_metadata
from .protocol import error_event, event, write_event
from .request import parse_request_file, validate_model_input


def _separate(request_file: Path) -> int:
    task_id = "unknown"
    try:
        request = parse_request_file(request_file)
        task_id = request.task_id
        validate_model_input(request.input_path)
        write_event(event("ready", task_id, {"device": request.device}))
        write_event(event("stage", task_id, {"stage": "loadingModel"}))
        write_event(event("stage", task_id, {"stage": "separating"}))
        separate(request, lambda current, total: write_event(event("progress", task_id, {"current": current, "total": total})))
        write_event(event("completed", task_id, {"outputPath": str(request.output_vocals_path)}))
    except BaseException as error:
        failure = map_exception(error)
        write_event(error_event(task_id, failure))
        return failure.exit_code
    return 0


def _self_test(checkpoint: Path | None, config_path: Path | None, device: str) -> int:
    try:
        if (checkpoint is None) != (config_path is None):
            raise WorkerFailure(ErrorCode.INVALID_REQUEST, "self-test requires both checkpoint and config paths")
        config = load_worker_config()
        if checkpoint is None:
            write_event(event("selfTest", "self-test", {"modelId": config.model_id, "status": config.status, "inferenceAvailable": False}))
            return 11
        metadata = load_model_metadata()
        if not checkpoint.is_file() or checkpoint.name != metadata.file_name:
            raise WorkerFailure(ErrorCode.INVALID_REQUEST, "self-test checkpoint does not match the pinned model file")
        model_config = load_kimberley_config(config_path)
        _load_runtime(model_config, checkpoint, device)
        write_event(event("selfTest", "self-test", {"modelId": config.model_id, "status": "READY", "inferenceAvailable": True, "device": device}))
        return 0
    except BaseException as error:
        failure = map_exception(error)
        write_event(error_event("self-test", failure))
        return failure.exit_code


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    separate_parser = commands.add_parser("separate")
    separate_parser.add_argument("--request", required=True, type=Path)
    self_test_parser = commands.add_parser("self-test")
    self_test_parser.add_argument("--checkpoint", type=Path)
    self_test_parser.add_argument("--config", dest="config_path", type=Path)
    self_test_parser.add_argument("--device", default="cuda:0", choices=["cuda:0"])
    arguments = parser.parse_args(argv)
    if arguments.command == "separate":
        return _separate(arguments.request)
    return _self_test(arguments.checkpoint, arguments.config_path, arguments.device)
