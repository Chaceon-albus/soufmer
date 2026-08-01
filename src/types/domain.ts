export type InputMode = "file" | "folder";
export type ProcessingMode = "compatibility44100" | "sourceSampleRate";
export type ConflictPolicy = "skip" | "overwrite" | "autoNumber";
export type OutputFormat = "flac" | "wavFloat32";

export interface StartBatchRequest {
  inputMode: InputMode;
  inputPath: string;
  outputDirectory: string;
  processingMode: ProcessingMode;
  generateBothModes: boolean;
  recursive: boolean;
  preserveDirectoryStructure: boolean;
  conflictPolicy: ConflictPolicy;
  outputFormat: OutputFormat;
}

export type EnvironmentStatus =
  | { type: "notInstalled"; estimatedDownloadBytes?: number; estimatedDiskBytes?: number }
  | { type: "installing"; runtimeVersion: string }
  | { type: "ready"; runtimeVersion: string; modelVersion: string; ffmpegVersion: string }
  | { type: "repairRequired"; reasonCode: string; estimatedDownloadBytes?: number; estimatedDiskBytes?: number }
  | { type: "unsupported"; reasonCode: string };

export interface ProgressValue { kind: "determinate" | "indeterminate"; fraction?: number }
export interface InitializationProgress { runtimeVersion: string; stepIndex: number; stepCount: number; stepId: string; overall: ProgressValue; current: ProgressValue; bytesCompleted?: number; bytesTotal?: number; bytesPerSecond?: number; detail?: string }
export interface BatchProgress { itemIndex: number; itemCount: number; currentInputPath: string; currentDisplayName: string; stage: string; overall: ProgressValue; current: ProgressValue; completedDurationSeconds: number; totalDurationSeconds: number; elapsedSeconds: number }
export interface BatchItemResult { itemIndex: number; inputPath: string; outputs: string[]; durationSeconds: number; warnings: string[]; errorCode?: string | null }
export interface BatchResult { taskId: string; outputDirectory: string; succeeded: number; failed: number; skipped: number; cancelled: boolean; items: BatchItemResult[] }
export interface AppError { code: string; stage: string; messageKey: string; recoverable: boolean; diagnosticId: string; itemPath?: string }
export interface AppSettings { schemaVersion: 1; locale: "zh-CN" | "en"; lastInputMode: InputMode; lastOutputDirectory?: string; processingMode: ProcessingMode; recursive: boolean; preserveDirectoryStructure: boolean; conflictPolicy: ConflictPolicy; outputFormat: OutputFormat; generateBothModes: boolean }
