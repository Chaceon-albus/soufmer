export interface AttributionEntry { name: string; statusKey: string; detailKey: string }

export const attributions: AttributionEntry[] = [
  { name: "uv", statusKey: "license.status.verified", detailKey: "license.uv" },
  { name: "Music-Source-Separation-Training", statusKey: "license.status.verified", detailKey: "license.msst" },
  { name: "KimberleyJSN MelBandRoformer", statusKey: "license.status.verified", detailKey: "license.model" },
  { name: "FFmpeg", statusKey: "license.status.verified", detailKey: "license.ffmpeg" },
];
