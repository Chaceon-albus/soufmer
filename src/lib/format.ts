export function formatBytes(bytes?: number): string {
  if (bytes === undefined || bytes === null || Number.isNaN(bytes) || bytes <= 0) return "—";
  return formatBinaryBytes(bytes);
}

export function formatBinaryBytes(bytes: number): string {
  if (!bytes || bytes <= 0 || Number.isNaN(bytes)) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index > 1 ? 1 : 0)} ${units[index]}`;
}

export function formatDuration(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${Math.floor(seconds % 60).toString().padStart(2, "0")}`;
}

export function formatRate(bytesPerSecond?: number): string {
  return bytesPerSecond && bytesPerSecond > 0 ? `${formatBytes(bytesPerSecond)}/s` : "—";
}
