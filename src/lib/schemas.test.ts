import { describe, expect, it } from "vitest";
import { startBatchRequestSchema } from "./schemas";

const request = { inputMode: "file", inputPath: "C:\\音乐\\song.flac", outputDirectory: "C:\\输出", processingMode: "compatibility44100", generateBothModes: false, recursive: true, preserveDirectoryStructure: true, conflictPolicy: "skip", outputFormat: "flac" };
describe("startBatchRequestSchema", () => {
  it("accepts a complete request and rejects missing paths", () => {
    expect(startBatchRequestSchema.safeParse(request).success).toBe(true);
    expect(startBatchRequestSchema.safeParse({ ...request, outputDirectory: " " }).error?.issues[0]?.message).toBe("validation.outputRequired");
  });
});
