import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { InitializationActivity, InitializationActivityEntry } from "@/types/domain";
import { UvTerminal } from "./uv-terminal";
import { formatUvTerminalActivity, isUvTerminalAtTail, toUvTerminalLines, UV_TERMINAL_COLORS, UV_TERMINAL_EMPTY_MESSAGE } from "./uv-terminal-model";

const activity = (message: string, fields: Partial<InitializationActivity> = {}): InitializationActivity => ({
  stepId: "syncingEnvironment",
  level: "status",
  message,
  ...fields,
});

describe("uv terminal", () => {
  it("formats only the approved Step 4 uv transcript lines", () => {
    const entries: InitializationActivityEntry[] = [
      { sequence: 1, activity: activity("resolvedPackages", { completedUnits: 45 }) },
      { sequence: 2, activity: activity("downloadingPackage", { level: "download", packageName: "torch", packageSizeBytes: 2_576_980_378 }) },
      { sequence: 3, activity: activity("downloadedPackage", { level: "download", packageName: "torch" }) },
      { sequence: 4, activity: activity("preparedPackages", { level: "install", completedUnits: 45 }) },
      { sequence: 5, activity: activity("installedPackage", { level: "install", packageName: "torch", packageVersion: "2.6.0+cu124" }) },
      { sequence: 6, activity: activity("installedPackages", { level: "install", completedUnits: 45 }) },
      { sequence: 7, activity: activity("resolvingPackages", { completedUnits: 45 }) },
      { sequence: 8, activity: activity("installedPython", { level: "install", packageName: "Python 3.11.9" }) },
      { sequence: 9, activity: activity("resolvedPackages", { stepId: "installingPython", completedUnits: 45 }) },
    ];

    expect(toUvTerminalLines(entries).map((line) => line.text)).toEqual([
      "Resolved 45 packages",
      "Downloading torch (2.4 GiB)",
      "Downloaded torch",
      "Prepared 45 packages",
      "+ torch==2.6.0+cu124",
      "Installed 45 packages",
    ]);
  });

  it("has a stable empty state and omits incomplete approved events", () => {
    expect(UV_TERMINAL_EMPTY_MESSAGE).toBe("Waiting for uv output...");
    expect(toUvTerminalLines([])).toEqual([]);
    expect(formatUvTerminalActivity(activity("downloadingPackage"))).toBeUndefined();
    expect(formatUvTerminalActivity(activity("installedPackage", { packageName: "torch" }))).toBeUndefined();
    expect(formatUvTerminalActivity(activity("preparedPackages"))).toBeUndefined();
  });

  it("renders the fixed Solarized Light waiting surface", () => {
    const markup = renderToStaticMarkup(createElement(UvTerminal, { activities: [] }));

    expect(markup).toContain("h-36 w-full");
    expect(markup).toContain("bg-[#fdf6e3]");
    expect(markup).toContain('role="log"');
    expect(markup).toContain(UV_TERMINAL_EMPTY_MESSAGE);
  });

  it("follows output only while the viewport remains at the tail", () => {
    expect(isUvTerminalAtTail(300, 156, 144)).toBe(true);
    expect(isUvTerminalAtTail(300, 148, 144)).toBe(false);
  });

  it("maps activity levels to Solarized Light line colors", () => {
    expect(UV_TERMINAL_COLORS).toEqual({
      status: "#586e75",
      download: "#268bd2",
      install: "#859900",
      warning: "#cb4b16",
    });
  });
});
