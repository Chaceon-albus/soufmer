import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { z } from "zod";
import { appSettingsSchema, backendErrorSchema, batchResultSchema, diagnosticReportSchema, environmentStatusSchema, taskAcknowledgementSchema } from "@/types/backend";
import type { AppError, AppSettings, BatchResult, EnvironmentStatus, StartBatchRequest } from "@/types/domain";
import { defaultSettings, mockEnvironment } from "@/app/app-state";

export const eventNames = ["runtime://progress", "runtime://completed", "batch://progress", "batch://item-completed", "batch://completed", "task://failed", "task://cancelled"] as const;
export const supportedAudioExtensions = ["wav", "flac", "mp3", "m4a", "aac", "ogg", "opus", "aiff", "aif", "wma"] as const;
export type BackendEventName = (typeof eventNames)[number];
export const isDesktopBridge = () => isTauri();

function toError(value: unknown): AppError {
  const parsed = backendErrorSchema.safeParse(value);
  if (parsed.success) return parsed.data;
  return { code: "IPC_FAILED", stage: "ipc", messageKey: "error.generic", recoverable: true, diagnosticId: "IPC_FAILED" };
}
export function toAppError(value: unknown): AppError { return toError(value); }

async function command<T>(name: string, args: Record<string, unknown> | undefined, schema: z.ZodType<T>): Promise<T> {
  if (!isDesktopBridge()) throw new Error("TAURI_UNAVAILABLE");
  const response: unknown = await invoke(name, args);
  return schema.parse(response);
}
export async function getEnvironmentStatus(): Promise<EnvironmentStatus> {
  if (!isDesktopBridge()) return mockEnvironment;
  return command("get_environment_status", undefined, environmentStatusSchema);
}
export async function getAppSettings(): Promise<AppSettings> {
  if (!isDesktopBridge()) return defaultSettings;
  return command("get_app_settings", undefined, appSettingsSchema);
}
export async function saveAppSettings(settings: AppSettings): Promise<void> {
  if (!isDesktopBridge()) return;
  await command("save_app_settings", { settings }, appSettingsSchema);
}
export async function initializeEnvironment() { return isDesktopBridge() ? command("initialize_environment", undefined, taskAcknowledgementSchema) : { taskId: "browser-initialization" }; }
export async function startBatch(request: StartBatchRequest) { return isDesktopBridge() ? command("start_batch", { request }, taskAcknowledgementSchema) : { taskId: "browser-batch" }; }
export async function cancelActiveTask(taskId: string) { if (isDesktopBridge()) await command("cancel_active_task", { request: { taskId } }, z.null()); }
export async function getDiagnosticReport(diagnosticId: string): Promise<string> { return command("get_diagnostic_report", { diagnosticId: z.string().min(1).parse(diagnosticId) }, diagnosticReportSchema); }
export async function chooseInputFile() { return isDesktopBridge() ? open({ multiple: false, directory: false, filters: [{ name: "Audio", extensions: [...supportedAudioExtensions] }] }) : null; }
export async function chooseFolder() { return isDesktopBridge() ? open({ multiple: false, directory: true }) : null; }
export async function chooseOutputDirectory() { return chooseFolder(); }
export async function revealOutputDirectory(path: string) { if (isDesktopBridge() && path) await revealItemInDir(path); }
export async function subscribe(name: BackendEventName, handler: (payload: unknown) => void): Promise<UnlistenFn> { return isDesktopBridge() ? listen(name, (event) => handler(event.payload)) : () => undefined; }
export function toBatchResult(value: unknown): BatchResult { return batchResultSchema.parse(value); }
