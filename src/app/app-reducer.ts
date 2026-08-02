import type { AppError, BatchProgress, BatchResult, EnvironmentStatus, InitializationActivity, InitializationProgress, StartBatchRequest } from "@/types/domain";
import { defaultSettings, mockEnvironment, type AppState } from "./app-state";

const MAX_INITIALIZATION_ACTIVITIES = 100;

export type AppAction =
  | { type: "booted"; environment: EnvironmentStatus; settings: Extract<AppState, { type: "idle" }> ["settings"] }
  | { type: "startRequested"; request: StartBatchRequest }
  | { type: "settingsUpdated"; settings: Extract<AppState, { type: "idle" }> ["settings"] }
  | { type: "initializationRequested" }
  | { type: "validationNeedsInitialization"; environment: EnvironmentStatus }
  | { type: "environmentNotReady"; environment: EnvironmentStatus }
  | { type: "validationPassed"; taskId: string; progress: BatchProgress }
  | { type: "initializationAccepted"; taskId: string; progress: InitializationProgress }
  | { type: "initializationProgress"; taskId: string; sequence: number; progress: InitializationProgress }
  | { type: "initializationActivity"; taskId: string; sequence: number; activity: InitializationActivity }
  | { type: "initializationCompleted"; taskId: string; sequence: number; environment: Extract<EnvironmentStatus, { type: "ready" }> }
  | { type: "processingStarted"; taskId: string; progress: BatchProgress }
  | { type: "batchProgress"; taskId: string; sequence: number; progress: BatchProgress }
  | { type: "cancelRequested" }
  | { type: "taskCancelled"; taskId: string }
  | { type: "completed"; result: BatchResult }
  | { type: "eventCompleted"; taskId: string; result: BatchResult }
  | { type: "eventFailed"; taskId: string; error: AppError }
  | { type: "initializationRetryRequested" }
  | { type: "developmentCompleted"; result: BatchResult }
  | { type: "failed"; error: AppError }
  | { type: "dismissed"; environment: EnvironmentStatus; settings: Extract<AppState, { type: "idle" }> ["settings"] };

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "booted": return state.type === "booting" ? { type: "idle", environment: action.environment, settings: action.settings } : state;
    case "startRequested": return state.type === "idle" ? { type: "validating", request: action.request, environment: state.environment, settings: state.settings } : state;
    case "settingsUpdated": return state.type === "booting" || sameSettings(state.settings, action.settings) ? state : { ...state, settings: action.settings };
    case "initializationRequested": return state.type === "idle" ? { type: "awaitingInitializationConsent", request: { inputMode: "file", inputPath: "", outputDirectory: "", processingMode: state.settings.processingMode, generateBothModes: state.settings.generateBothModes, recursive: state.settings.recursive, preserveDirectoryStructure: state.settings.preserveDirectoryStructure, conflictPolicy: state.settings.conflictPolicy, outputFormat: state.settings.outputFormat }, environment: state.environment, settings: state.settings } : state;
    case "validationNeedsInitialization": return state.type === "validating" ? { type: "awaitingInitializationConsent", request: state.request, environment: action.environment, settings: state.settings } : state;
    case "validationPassed": return state.type === "validating" ? { type: "processing", taskId: action.taskId, environment: state.environment, settings: state.settings, progress: action.progress, lastSequence: 0, outputDirectory: state.request.outputDirectory } : state;
    case "initializationAccepted": return state.type === "awaitingInitializationConsent" ? { type: "initializing", taskId: action.taskId, request: state.request, environment: state.environment, settings: state.settings, progress: action.progress, activities: [], lastSequence: 0 } : state;
    case "initializationProgress": return state.type === "initializing" && state.taskId === action.taskId && action.sequence > state.lastSequence ? { ...state, progress: monotonicInitializationProgress(state.progress, action.progress), lastSequence: action.sequence } : state;
    case "initializationActivity": return state.type === "initializing" && state.taskId === action.taskId && action.sequence > state.lastSequence ? { ...state, activities: [...state.activities, { sequence: action.sequence, activity: action.activity }].slice(-MAX_INITIALIZATION_ACTIVITIES), lastSequence: action.sequence } : state;
    case "initializationCompleted": return state.type === "initializing" && state.taskId === action.taskId && action.sequence > state.lastSequence ? state.request.inputPath.trim() && state.request.outputDirectory.trim() ? { type: "validating", request: state.request, environment: action.environment, settings: state.settings } : { type: "idle", environment: action.environment, settings: state.settings } : state;
    case "processingStarted": return state.type === "initializing" ? { type: "processing", taskId: action.taskId, environment: state.environment, settings: state.settings, progress: action.progress, lastSequence: 0, outputDirectory: state.request.outputDirectory } : state;
    case "batchProgress": return state.type === "processing" && state.taskId === action.taskId && action.sequence > state.lastSequence ? { ...state, progress: action.progress, lastSequence: action.sequence } : state;
    case "cancelRequested": return state.type === "initializing" || state.type === "processing" ? { type: "cancelling", taskId: state.taskId, environment: state.environment, settings: state.settings, outputDirectory: state.type === "processing" ? state.outputDirectory : undefined, initializationRequest: state.type === "initializing" ? state.request : undefined, lastProgress: state.progress, initializationActivities: state.type === "initializing" ? state.activities : undefined } : state;
    case "taskCancelled": return state.type === "cancelling" && state.taskId === action.taskId ? state.lastProgress && "stepIndex" in state.lastProgress ? { type: "idle", environment: state.environment, settings: state.settings } : { type: "completed", environment: state.environment, settings: state.settings, result: { taskId: action.taskId, succeeded: 0, failed: 0, skipped: 0, outputDirectory: state.outputDirectory ?? "", cancelled: true, items: [] } } : state;
    case "completed": return state.type === "initializing" || state.type === "processing" || state.type === "cancelling" ? { type: "completed", environment: state.environment, settings: state.settings, result: action.result } : state;
    case "eventCompleted": return (state.type === "processing" || state.type === "cancelling") && state.taskId === action.taskId ? { type: "completed", environment: state.environment, settings: state.settings, result: action.result } : state;
    case "developmentCompleted": return state.type === "booting" ? state : { type: "completed", environment: state.environment, settings: state.settings, result: action.result };
    case "failed": return state.type === "booting" ? { type: "failed", error: action.error, environment: mockEnvironment, settings: defaultSettings } : { type: "failed", error: action.error, environment: state.environment, settings: state.settings, initializationRequest: state.type === "awaitingInitializationConsent" || state.type === "initializing" ? state.request : state.type === "cancelling" ? state.initializationRequest : undefined };
    case "eventFailed": return (state.type === "initializing" || state.type === "processing" || state.type === "cancelling") && state.taskId === action.taskId ? { type: "failed", error: action.error, environment: state.environment, settings: state.settings, initializationRequest: state.type === "initializing" ? state.request : state.type === "cancelling" ? state.initializationRequest : undefined } : state;
    case "initializationRetryRequested": return state.type === "failed" && state.error.recoverable && state.initializationRequest ? { type: "awaitingInitializationConsent", request: state.initializationRequest, environment: state.environment, settings: state.settings } : state;
    case "environmentNotReady": return state.type === "validating" ? { type: "awaitingInitializationConsent", request: state.request, environment: action.environment, settings: state.settings } : state.type === "initializing" ? { type: "awaitingInitializationConsent", request: state.request, environment: action.environment, settings: state.settings } : state;
    case "dismissed": return state.type === "completed" || state.type === "failed" ? { type: "idle", environment: action.environment, settings: action.settings } : state;
  }
}

function monotonicInitializationProgress(previous: InitializationProgress, next: InitializationProgress): InitializationProgress {
  const overall = monotonicProgressValue(previous.overall, next.overall);
  const current = previous.stepId === next.stepId ? monotonicProgressValue(previous.current, next.current) : next.current;
  return {
    ...next,
    overall,
    current,
  };
}

function monotonicProgressValue(previous: InitializationProgress["overall"], next: InitializationProgress["overall"]): InitializationProgress["overall"] {
  if (previous.kind !== "determinate") return next;
  if (next.kind !== "determinate") return previous;
  return { kind: "determinate", fraction: Math.max(previous.fraction ?? 0, next.fraction ?? 0) };
}

function sameSettings(left: Extract<AppState, { type: "idle" }> ["settings"], right: Extract<AppState, { type: "idle" }> ["settings"]) {
  return left.schemaVersion === right.schemaVersion && left.locale === right.locale && left.lastInputMode === right.lastInputMode && left.lastOutputDirectory === right.lastOutputDirectory && left.processingMode === right.processingMode && left.recursive === right.recursive && left.preserveDirectoryStructure === right.preserveDirectoryStructure && left.conflictPolicy === right.conflictPolicy && left.outputFormat === right.outputFormat && left.generateBothModes === right.generateBothModes;
}
