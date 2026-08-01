import type { AppError, AppSettings, BatchProgress, BatchResult, EnvironmentStatus, InitializationProgress, StartBatchRequest } from "@/types/domain";

export type AppState =
  | { type: "booting" }
  | { type: "idle"; environment: EnvironmentStatus; settings: AppSettings }
  | { type: "validating"; request: StartBatchRequest; environment: EnvironmentStatus; settings: AppSettings }
  | { type: "awaitingInitializationConsent"; request: StartBatchRequest; environment: EnvironmentStatus; settings: AppSettings }
  | { type: "initializing"; taskId: string; request: StartBatchRequest; environment: EnvironmentStatus; settings: AppSettings; progress: InitializationProgress; lastSequence: number }
  | { type: "processing"; taskId: string; environment: EnvironmentStatus; settings: AppSettings; progress: BatchProgress; lastSequence: number; outputDirectory: string }
  | { type: "cancelling"; taskId: string; environment: EnvironmentStatus; settings: AppSettings; outputDirectory?: string; lastProgress?: BatchProgress | InitializationProgress }
  | { type: "completed"; result: BatchResult; environment: EnvironmentStatus; settings: AppSettings }
  | { type: "failed"; error: AppError; environment: EnvironmentStatus; settings: AppSettings };

export const defaultSettings: AppSettings = { schemaVersion: 1, locale: "zh-CN", lastInputMode: "file", processingMode: "compatibility44100", recursive: true, preserveDirectoryStructure: true, conflictPolicy: "skip", outputFormat: "flac", generateBothModes: false };
export const mockEnvironment: EnvironmentStatus = { type: "notInstalled", estimatedDownloadBytes: 3_800_000_000, estimatedDiskBytes: 7_000_000_000 };
