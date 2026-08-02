import { useEffect, useRef } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { InitializationActivityEntry } from "@/types/domain";
import { isUvTerminalAtTail, toUvTerminalLines, UV_TERMINAL_COLORS, UV_TERMINAL_EMPTY_MESSAGE } from "./uv-terminal-model";

export function UvTerminal({ activities }: { activities: InitializationActivityEntry[] }) {
  const lines = toUvTerminalLines(activities);
  const viewportRef = useRef<HTMLDivElement>(null);
  const followLatest = useRef(true);
  const latestSequence = lines.at(-1)?.sequence;

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || !followLatest.current) return;
    const frame = requestAnimationFrame(() => viewport.scrollTo({ top: viewport.scrollHeight }));
    return () => cancelAnimationFrame(frame);
  }, [latestSequence]);

  return <ScrollArea
    className="h-36 w-full rounded-md border border-[#eee8d5] bg-[#fdf6e3] font-mono"
    viewportRef={viewportRef}
    onViewportScroll={(event) => {
      const viewport = event.currentTarget;
      followLatest.current = isUvTerminalAtTail(viewport.scrollHeight, viewport.scrollTop, viewport.clientHeight);
    }}
  >
    <div role="log" aria-label="uv output" aria-live="polite" className="min-h-full p-3 text-xs leading-5">
      {lines.length === 0
        ? <p className="text-[#93a1a1]">{UV_TERMINAL_EMPTY_MESSAGE}</p>
        : lines.map((line) => <p key={line.sequence} data-level={line.level} className="break-all" style={{ color: UV_TERMINAL_COLORS[line.level] }}>{line.text}</p>)}
    </div>
  </ScrollArea>;
}
