import { describe, expect, it } from "vitest";
import {
  backendEventSchema,
  diagnosticReportSchema,
  environmentStatusSchema,
  initializationProgressSchema,
  licenseNoticesSchema,
  readyEnvironmentSchema,
  taskAcknowledgementSchema,
} from "./backend";

describe("backend event schemas", () => {
  it("requires the expected channel type and accepts a ready runtime completion envelope", () => {
    const schema = backendEventSchema("runtime://completed", readyEnvironmentSchema);
    const event = {
      schemaVersion: 1,
      taskId: "runtime-1",
      sequence: 3,
      timestamp: "2026-07-30T12:00:00Z",
      type: "runtime://completed",
      payload: { type: "ready", runtimeVersion: "1.0.0", modelVersion: "model", ffmpegVersion: "ffmpeg" },
    };

    expect(schema.safeParse(event).success).toBe(true);
    expect(schema.safeParse({ ...event, type: "runtime://progress" }).success).toBe(false);
  });

  it("requires acknowledgement acceptance time and normalizes absent runtime progress detail", () => {
    expect(taskAcknowledgementSchema.safeParse({ taskId: "task-1" }).success).toBe(false);
    expect(initializationProgressSchema.parse({
      runtimeVersion: "1.0.0",
      stepIndex: 1,
      stepCount: 7,
      stepId: "checkingSystem",
      overall: { kind: "indeterminate" },
      current: { kind: "indeterminate" },
      detail: null,
    }).detail).toBeUndefined();
  });

  it("accepts only nonempty persisted diagnostic reports", () => {
    expect(diagnosticReportSchema.safeParse("runtime log").success).toBe(true);
    expect(diagnosticReportSchema.safeParse("").success).toBe(false);
  });

  it("requires strict, nonempty embedded license notices", () => {
    const notice = { id: "uv-mit", title: "uv — MIT License", text: "MIT License" };

    expect(licenseNoticesSchema.safeParse([notice]).success).toBe(true);
    expect(licenseNoticesSchema.safeParse([]).success).toBe(false);
    expect(licenseNoticesSchema.safeParse([{ ...notice, source: "runtime" }]).success).toBe(false);
    expect(licenseNoticesSchema.safeParse([{ ...notice, text: "" }]).success).toBe(false);
  });

  it("accepts manifest estimates for installation and repair statuses", () => {
    for (const status of [
      { type: "notInstalled", estimatedDownloadBytes: 3_500_000_000, estimatedDiskBytes: 7_000_000_000 },
      { type: "repairRequired", reasonCode: "RUNTIME_VALIDATION_FAILED", estimatedDownloadBytes: 3_500_000_000, estimatedDiskBytes: 7_000_000_000 },
    ]) {
      expect(environmentStatusSchema.safeParse(status).success).toBe(true);
    }
    const snakeCase = environmentStatusSchema.parse({ type: "repairRequired", reasonCode: "RUNTIME_VALIDATION_FAILED", estimated_download_bytes: 1 });
    expect("estimatedDownloadBytes" in snakeCase).toBe(false);
    expect(environmentStatusSchema.safeParse({ type: "notInstalled", estimatedDownloadBytes: -1 }).success).toBe(false);
  });
});
