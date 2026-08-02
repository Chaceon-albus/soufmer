import { describe, expect, it } from "vitest";
import { formatBinaryBytes, formatBytes, formatDuration, formatRate } from "./format";

describe("format utilities", () => {
  it("formats bytes consistently using binary IEC units", () => {
    expect(formatBytes(0)).toBe("—");
    expect(formatBytes(undefined)).toBe("—");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe("1 KiB");
    expect(formatBytes(1048576)).toBe("1.0 MiB");
    expect(formatBytes(2576980378)).toBe("2.4 GiB");
  });

  it("formats binary bytes consistently", () => {
    expect(formatBinaryBytes(0)).toBe("0 B");
    expect(formatBinaryBytes(1024)).toBe("1 KiB");
    expect(formatBinaryBytes(2576980378)).toBe("2.4 GiB");
  });

  it("formats rate consistently", () => {
    expect(formatRate(0)).toBe("—");
    expect(formatRate(undefined)).toBe("—");
    expect(formatRate(10485760)).toBe("10.0 MiB/s");
    expect(formatRate(2576980378)).toBe("2.4 GiB/s");
  });

  it("formats duration", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(65)).toBe("1:05");
    expect(formatDuration(3665)).toBe("61:05");
  });
});
