import { z } from "zod";

export const startBatchRequestSchema = z.object({
  inputMode: z.enum(["file", "folder"]),
  inputPath: z.string().trim().min(1, "validation.inputRequired"),
  outputDirectory: z.string().trim().min(1, "validation.outputRequired"),
  processingMode: z.enum(["compatibility44100", "sourceSampleRate"]),
  generateBothModes: z.boolean(),
  recursive: z.boolean(),
  preserveDirectoryStructure: z.boolean(),
  conflictPolicy: z.enum(["skip", "overwrite", "autoNumber"]),
  outputFormat: z.enum(["flac", "wavFloat32"]),
});
