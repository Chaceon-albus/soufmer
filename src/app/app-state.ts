import type { AppError, AppSettings, BatchProgress, BatchResult, EnvironmentStatus, InitializationActivityEntry, InitializationProgress, StartBatchRequest } from "@/types/domain";

export type AppState =
  | { type: "booting" }
  | { type: "idle"; environment: EnvironmentStatus; settings: AppSettings }
  | { type: "validating"; request: StartBatchRequest; environment: EnvironmentStatus; settings: AppSettings }
  | { type: "awaitingInitializationConsent"; request: StartBatchRequest; environment: EnvironmentStatus; settings: AppSettings }
  | { type: "initializing"; taskId: string; request: StartBatchRequest; environment: EnvironmentStatus; settings: AppSettings; progress: InitializationProgress; activities: InitializationActivityEntry[]; lastSequence: number }
  | { type: "processing"; taskId: string; environment: EnvironmentStatus; settings: AppSettings; progress: BatchProgress; lastSequence: number; outputDirectory: string }
  | { type: "cancelling"; taskId: string; environment: EnvironmentStatus; settings: AppSettings; outputDirectory?: string; initializationRequest?: StartBatchRequest; lastProgress?: BatchProgress | InitializationProgress; initializationActivities?: InitializationActivityEntry[] }
  | { type: "completed"; result: BatchResult; environment: EnvironmentStatus; settings: AppSettings }
  | { type: "failed"; error: AppError; environment: EnvironmentStatus; settings: AppSettings; initializationRequest?: StartBatchRequest };

export function detectSystemLocale(): "zh-CN" | "en" {
  if (typeof navigator !== "undefined" && navigator.language) {
    if (navigator.language.toLowerCase().startsWith("zh")) {
      return "zh-CN";
    }
    return "en";
  }
  return "zh-CN";
}

export const defaultSettings: AppSettings = { schemaVersion: 1, locale: detectSystemLocale(), lastInputMode: "file", processingMode: "compatibility44100", recursive: true, preserveDirectoryStructure: true, conflictPolicy: "skip", outputFormat: "flac", generateBothModes: false };
export const mockEnvironment: EnvironmentStatus = { type: "notInstalled", estimatedDownloadBytes: 3_800_000_000, estimatedDiskBytes: 7_000_000_000 };
