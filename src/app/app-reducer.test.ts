import { describe, expect, it } from "vitest";
import { appReducer } from "./app-reducer";
import { defaultSettings, mockEnvironment, type AppState } from "./app-state";

const request = { inputMode: "file" as const, inputPath: "C:\\song.flac", outputDirectory: "C:\\output", processingMode: "compatibility44100" as const, generateBothModes: false, recursive: true, preserveDirectoryStructure: true, conflictPolicy: "skip" as const, outputFormat: "flac" as const };
const progress = { itemIndex: 1, itemCount: 1, currentInputPath: request.inputPath, currentDisplayName: "song.flac", stage: "separating", overall: { kind: "determinate" as const, fraction: 0.5 }, current: { kind: "determinate" as const, fraction: 0.5 }, completedDurationSeconds: 10, totalDurationSeconds: 20, elapsedSeconds: 10 };

describe("appReducer", () => {
  it("follows the normal request to completion lifecycle", () => {
    let state: AppState = { type: "booting" };
    state = appReducer(state, { type: "booted", environment: mockEnvironment, settings: defaultSettings });
    state = appReducer(state, { type: "startRequested", request });
    state = appReducer(state, { type: "validationPassed", taskId: "task-1", progress });
    state = appReducer(state, { type: "completed", result: { taskId: "task-1", succeeded: 1, failed: 0, skipped: 0, outputDirectory: request.outputDirectory, cancelled: false, items: [] } });
    expect(state).toMatchObject({ type: "completed", result: { succeeded: 1 } });
  });
  it("ignores stale progress from another task or an earlier sequence", () => {
    const state: AppState = { type: "processing", taskId: "task-1", environment: mockEnvironment, settings: defaultSettings, progress, lastSequence: 4, outputDirectory: request.outputDirectory };
    expect(appReducer(state, { type: "batchProgress", taskId: "task-2", sequence: 5, progress: { ...progress, itemIndex: 2 } })).toBe(state);
    expect(appReducer(state, { type: "batchProgress", taskId: "task-1", sequence: 4, progress: { ...progress, itemIndex: 2 } })).toBe(state);
  });
  it("keeps the latest settings through completion and dismissal", () => {
    const changedSettings = { ...defaultSettings, processingMode: "sourceSampleRate" as const, lastOutputDirectory: "C:\\chosen-output" };
    let state: AppState = { type: "idle", environment: mockEnvironment, settings: defaultSettings };
    state = appReducer(state, { type: "settingsUpdated", settings: changedSettings });
    state = appReducer(state, { type: "startRequested", request: { ...request, processingMode: changedSettings.processingMode, outputDirectory: changedSettings.lastOutputDirectory ?? request.outputDirectory } });
    state = appReducer(state, { type: "validationPassed", taskId: "task-2", progress });
    state = appReducer(state, { type: "completed", result: { taskId: "task-2", succeeded: 1, failed: 0, skipped: 0, outputDirectory: request.outputDirectory, cancelled: false, items: [] } });
    if (state.type !== "completed") throw new Error("expected completed state");
    state = appReducer(state, { type: "dismissed", environment: state.environment, settings: state.settings });
    expect(state).toMatchObject({ type: "idle", settings: changedSettings });
  });
  it("returns to validation after initialization when a preserved request is complete", () => {
    const initializationProgress = {
      runtimeVersion: "1.0.0",
      stepIndex: 1,
      stepCount: 7,
      stepId: "checkingSystem" as const,
      overall: { kind: "determinate" as const, fraction: 0.1 },
      current: { kind: "indeterminate" as const },
    };
    const readyEnvironment = { type: "ready" as const, runtimeVersion: "1.0.0", modelVersion: "model", ffmpegVersion: "ffmpeg" };
    const initializing: AppState = { type: "initializing", taskId: "runtime-1", request, environment: { type: "notInstalled" }, settings: defaultSettings, progress: initializationProgress, lastSequence: 1 };

    expect(appReducer(initializing, { type: "initializationCompleted", taskId: "runtime-1", sequence: 2, environment: readyEnvironment })).toMatchObject({
      type: "validating",
      request,
      environment: readyEnvironment,
    });

    const blankRequest = { ...request, inputPath: " ", outputDirectory: "" };
    const blankInitializing: AppState = { ...initializing, request: blankRequest };
    expect(appReducer(blankInitializing, { type: "initializationCompleted", taskId: "runtime-1", sequence: 2, environment: readyEnvironment })).toMatchObject({
      type: "idle",
      environment: readyEnvironment,
    });
  });
  it("returns initialization cancellation to idle and only fabricates a batch result as a fallback", () => {
    const initializationProgress = {
      runtimeVersion: "1.0.0",
      stepIndex: 1,
      stepCount: 7,
      stepId: "checkingSystem" as const,
      overall: { kind: "determinate" as const, fraction: 0.1 },
      current: { kind: "indeterminate" as const },
    };
    const cancellingInitialization: AppState = { type: "cancelling", taskId: "runtime-1", environment: mockEnvironment, settings: defaultSettings, lastProgress: initializationProgress };
    expect(appReducer(cancellingInitialization, { type: "taskCancelled", taskId: "runtime-1" })).toMatchObject({ type: "idle" });

    const cancellingBatch: AppState = { type: "cancelling", taskId: "batch-1", environment: mockEnvironment, settings: defaultSettings, outputDirectory: request.outputDirectory, lastProgress: progress };
    const completed = appReducer(cancellingBatch, { type: "eventCompleted", taskId: "batch-1", result: { taskId: "batch-1", succeeded: 1, failed: 0, skipped: 0, outputDirectory: request.outputDirectory, cancelled: true, items: [] } });
    expect(completed).toMatchObject({
      type: "completed",
      result: { succeeded: 1, cancelled: true },
    });
    expect(appReducer(completed, { type: "taskCancelled", taskId: "batch-1" })).toBe(completed);

    expect(appReducer(cancellingBatch, { type: "taskCancelled", taskId: "batch-1" })).toMatchObject({
      type: "completed",
      result: { cancelled: true, outputDirectory: request.outputDirectory },
    });
  });
});
