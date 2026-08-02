import { formatBinaryBytes } from "@/lib/format";
import type { InitializationActivity, InitializationActivityEntry } from "@/types/domain";

export const UV_TERMINAL_EMPTY_MESSAGE = "Waiting for uv output...";

export const UV_TERMINAL_COLORS = {
  status: "#586e75",
  download: "#268bd2",
  install: "#859900",
  warning: "#cb4b16",
} as const;

export type UvTerminalLine = {
  sequence: number;
  level: InitializationActivity["level"];
  text: string;
};

export function toUvTerminalLines(activities: InitializationActivityEntry[]): UvTerminalLine[] {
  return activities.flatMap(({ sequence, activity }) => {
    if (activity.stepId !== "syncingEnvironment") return [];
    const text = formatUvTerminalActivity(activity);
    return text ? [{ sequence, level: activity.level, text }] : [];
  });
}

export function formatUvTerminalActivity(activity: InitializationActivity): string | undefined {
  const count = activity.completedUnits ?? activity.totalUnits;
  switch (activity.message) {
    case "resolvedPackages":
      return isSafeCount(count) ? `Resolved ${count} ${packageLabel(count)}` : undefined;
    case "downloadingPackage":
      if (!activity.packageName) return undefined;
      return activity.packageSizeBytes
        ? `Downloading ${activity.packageName} (${formatBinaryBytes(activity.packageSizeBytes)})`
        : `Downloading ${activity.packageName}`;
    case "downloadedPackage":
      return activity.packageName ? `Downloaded ${activity.packageName}` : undefined;
    case "preparedPackages":
      return isSafeCount(count) ? `Prepared ${count} ${packageLabel(count)}` : undefined;
    case "installedPackage":
      return activity.packageName && activity.packageVersion
        ? `+ ${activity.packageName}==${activity.packageVersion}`
        : undefined;
    case "installedPackages":
      return isSafeCount(count) ? `Installed ${count} ${packageLabel(count)}` : undefined;
    default:
      return undefined;
  }
}

export function isUvTerminalAtTail(scrollHeight: number, scrollTop: number, clientHeight: number) {
  return scrollHeight - scrollTop - clientHeight < 8;
}

function isSafeCount(value: number | undefined): value is number {
  return value !== undefined && Number.isSafeInteger(value) && value >= 0;
}

function packageLabel(count: number) {
  return count === 1 ? "package" : "packages";
}
