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
    const initializing: AppState = { type: "initializing", taskId: "runtime-1", request, environment: { type: "notInstalled" }, settings: defaultSettings, progress: initializationProgress, activities: [], lastSequence: 1 };

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
  it("preserves a pending batch request across a recoverable initialization retry", () => {
    const initializationProgress = {
      runtimeVersion: "1.0.0",
      stepIndex: 1,
      stepCount: 7,
      stepId: "checkingSystem" as const,
      overall: { kind: "determinate" as const, fraction: 0.1 },
      current: { kind: "indeterminate" as const },
    };
    const readyEnvironment = { type: "ready" as const, runtimeVersion: "1.0.0", modelVersion: "model", ffmpegVersion: "ffmpeg" };
    const error = { code: "ENV_DOWNLOAD_FAILED", stage: "runtime", messageKey: "error.environmentDownloadFailed", recoverable: true, diagnosticId: "diagnostic-1" };
    let state: AppState = { type: "validating", request, environment: mockEnvironment, settings: defaultSettings };

    state = appReducer(state, { type: "validationNeedsInitialization", environment: { type: "notInstalled" } });
    state = appReducer(state, { type: "failed", error });
    expect(state).toMatchObject({ type: "failed", initializationRequest: request });

    state = appReducer(state, { type: "initializationRetryRequested" });
    expect(state).toMatchObject({ type: "awaitingInitializationConsent", request });
    state = appReducer(state, { type: "initializationAccepted", taskId: "runtime-retry", progress: initializationProgress });
    state = appReducer(state, { type: "eventFailed", taskId: "runtime-retry", error });
    expect(state).toMatchObject({ type: "failed", initializationRequest: request });

    state = appReducer(state, { type: "initializationRetryRequested" });
    state = appReducer(state, { type: "initializationAccepted", taskId: "runtime-retry-2", progress: initializationProgress });
    state = appReducer(state, { type: "initializationCompleted", taskId: "runtime-retry-2", sequence: 1, environment: readyEnvironment });

    expect(state).toMatchObject({ type: "validating", request, environment: readyEnvironment });
  });
  it("does not offer a retry transition for a non-recoverable initialization failure", () => {
    const state: AppState = {
      type: "failed",
      error: { code: "ENV_HASH_MISMATCH", stage: "runtime", messageKey: "error.environmentHashMismatch", recoverable: false, diagnosticId: "diagnostic-2" },
      environment: mockEnvironment,
      settings: defaultSettings,
      initializationRequest: request,
    };

    expect(appReducer(state, { type: "initializationRetryRequested" })).toBe(state);
  });
  it("orders and bounds initialization activity while preserving truthful progress", () => {
    const initializationProgress = {
      runtimeVersion: "1.0.0",
      stepIndex: 4,
      stepCount: 7,
      stepId: "syncingEnvironment",
      overall: { kind: "determinate" as const, fraction: 0.15 },
      current: { kind: "indeterminate" as const },
    };
    const activity = { stepId: "syncingEnvironment", level: "download" as const, message: "downloadingPackage", packageName: "torch" };
    let state: AppState = { type: "initializing", taskId: "runtime-1", request, environment: { type: "notInstalled" }, settings: defaultSettings, progress: initializationProgress, activities: [], lastSequence: 1 };

    expect(appReducer(state, { type: "initializationActivity", taskId: "other-task", sequence: 2, activity })).toBe(state);
    state = appReducer(state, { type: "initializationActivity", taskId: "runtime-1", sequence: 2, activity });
    expect(state).toMatchObject({ type: "initializing", lastSequence: 2, activities: [{ sequence: 2 }] });
    expect(appReducer(state, { type: "initializationProgress", taskId: "runtime-1", sequence: 2, progress: initializationProgress })).toBe(state);

    state = appReducer(state, {
      type: "initializationProgress",
      taskId: "runtime-1",
      sequence: 3,
      progress: { ...initializationProgress, overall: { kind: "determinate", fraction: 0.10 }, current: { kind: "determinate", fraction: 0.8 } },
    });
    state = appReducer(state, {
      type: "initializationProgress",
      taskId: "runtime-1",
      sequence: 4,
      progress: { ...initializationProgress, overall: { kind: "determinate", fraction: 0.12 }, current: { kind: "determinate", fraction: 0.2 } },
    });
    expect(state).toMatchObject({ progress: { overall: { fraction: 0.15 }, current: { kind: "determinate", fraction: 0.8 } } });

    state = appReducer(state, {
      type: "initializationProgress",
      taskId: "runtime-1",
      sequence: 5,
      progress: { ...initializationProgress, overall: { kind: "indeterminate" }, current: { kind: "indeterminate" } },
    });
    expect(state).toMatchObject({ progress: { overall: { fraction: 0.15 }, current: { kind: "determinate", fraction: 0.8 } } });

    state = appReducer(state, {
      type: "initializationProgress",
      taskId: "runtime-1",
      sequence: 6,
      progress: { ...initializationProgress, stepIndex: 5, stepId: "downloadingModel", overall: { kind: "determinate", fraction: 0.6 }, current: { kind: "determinate", fraction: 0.1 } },
    });
    expect(state).toMatchObject({ progress: { stepId: "downloadingModel", overall: { fraction: 0.6 }, current: { fraction: 0.1 } } });

    for (let sequence = 7; sequence <= 111; sequence += 1) {
      state = appReducer(state, { type: "initializationActivity", taskId: "runtime-1", sequence, activity: { ...activity, packageName: `package-${sequence}` } });
    }
    expect(state).toMatchObject({ type: "initializing", lastSequence: 111 });
    if (state.type !== "initializing") throw new Error("expected initializing state");
    expect(state.activities).toHaveLength(100);
    expect(state.activities[0].sequence).toBe(12);

    const cancelling = appReducer(state, { type: "cancelRequested" });
    if (cancelling.type !== "cancelling") throw new Error("expected cancelling state");
    expect(cancelling.initializationActivities).toBe(state.activities);
    expect(cancelling.initializationActivities).toHaveLength(100);
    expect(cancelling.initializationActivities?.[0].sequence).toBe(12);
    expect(appReducer(cancelling, { type: "initializationActivity", taskId: "runtime-1", sequence: 112, activity })).toBe(cancelling);
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
  it("dismisses awaitingInitializationConsent state back to idle", () => {
    const awaiting: AppState = { type: "awaitingInitializationConsent", request, environment: { type: "notInstalled" }, settings: defaultSettings };
    expect(appReducer(awaiting, { type: "dismissed", environment: awaiting.environment, settings: awaiting.settings })).toMatchObject({
      type: "idle",
      environment: { type: "notInstalled" },
    });
  });
});
