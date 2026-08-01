from accompaniment_worker.errors import ErrorCode, map_exception


def test_error_mapping_does_not_need_model_import() -> None:
    assert map_exception(RuntimeError("CUDA out of memory")).code is ErrorCode.CUDA_OUT_OF_MEMORY
    assert map_exception(ModuleNotFoundError("torch")).code is ErrorCode.RUNTIME_IMPORT_FAILED
