import { z } from "zod";

const progressValueSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("determinate"), fraction: z.number().finite().min(0).max(1) }),
  z.object({ kind: z.literal("indeterminate") }),
]);

export const backendErrorSchema = z.object({ code: z.string(), stage: z.string(), messageKey: z.string(), recoverable: z.boolean(), diagnosticId: z.string(), itemPath: z.string().optional() });
export const diagnosticReportSchema = z.string().min(1);
export const licenseNoticeSchema = z.object({ id: z.string().min(1), title: z.string().min(1), text: z.string().min(1) }).strict();
export const licenseNoticesSchema = z.array(licenseNoticeSchema).min(1);
export type LicenseNotice = z.infer<typeof licenseNoticeSchema>;
export const taskAcknowledgementSchema = z.object({ taskId: z.string().min(1), acceptedAt: z.string().min(1) });
export const readyEnvironmentSchema = z.object({ type: z.literal("ready"), runtimeVersion: z.string(), modelVersion: z.string(), ffmpegVersion: z.string() });
const byteEstimateSchema = z.number().finite().int().nonnegative();
export const environmentStatusSchema = z.discriminatedUnion("type", [z.object({ type: z.literal("notInstalled"), estimatedDownloadBytes: byteEstimateSchema.optional(), estimatedDiskBytes: byteEstimateSchema.optional() }), z.object({ type: z.literal("installing"), runtimeVersion: z.string() }), readyEnvironmentSchema, z.object({ type: z.literal("repairRequired"), reasonCode: z.string(), estimatedDownloadBytes: byteEstimateSchema.optional(), estimatedDiskBytes: byteEstimateSchema.optional() }), z.object({ type: z.literal("unsupported"), reasonCode: z.string() })]);
export const appSettingsSchema = z.object({ schemaVersion: z.literal(1), locale: z.enum(["zh-CN", "en"]), lastInputMode: z.enum(["file", "folder"]), lastOutputDirectory: z.string().optional(), processingMode: z.enum(["compatibility44100", "sourceSampleRate"]), recursive: z.boolean(), preserveDirectoryStructure: z.boolean(), conflictPolicy: z.enum(["skip", "overwrite", "autoNumber"]), outputFormat: z.enum(["flac", "wavFloat32"]), generateBothModes: z.boolean() });
export const initializationProgressSchema = z.object({ runtimeVersion: z.string(), stepIndex: z.number(), stepCount: z.number(), stepId: z.enum(["checkingSystem", "preparingTools", "installingPython", "syncingEnvironment", "downloadingModel", "selfTesting", "activating"]), overall: progressValueSchema, current: progressValueSchema, bytesCompleted: z.number().optional(), bytesTotal: z.number().optional(), bytesPerSecond: z.number().optional(), detail: z.string().nullable().optional().transform((value) => value ?? undefined) });
export const batchProgressSchema = z.object({ itemIndex: z.number(), itemCount: z.number(), currentInputPath: z.string(), currentDisplayName: z.string(), stage: z.enum(["probing", "preparingInput", "separating", "buildingCompatibilityOutput", "buildingSourceRateOutput", "validatingOutput", "cleaningUp"]), overall: progressValueSchema, current: progressValueSchema, completedDurationSeconds: z.number(), totalDurationSeconds: z.number(), elapsedSeconds: z.number() });
export const batchItemSchema = z.object({ itemIndex: z.number(), inputPath: z.string(), outputs: z.array(z.string()), durationSeconds: z.number(), warnings: z.array(z.string()), errorCode: z.string().nullable().optional() });
export const batchResultSchema = z.object({ taskId: z.string(), outputDirectory: z.string(), succeeded: z.number(), failed: z.number(), skipped: z.number(), cancelled: z.boolean(), items: z.array(batchItemSchema) });
export const backendEventSchema = <T extends z.ZodType>(eventType: string, payload: T) => z.object({ schemaVersion: z.literal(1), taskId: z.string(), sequence: z.number().int().nonnegative(), timestamp: z.string(), type: z.literal(eventType), payload });

export type BackendEvent<T> = { schemaVersion: 1; taskId: string; sequence: number; timestamp: string; type: string; payload: T };
