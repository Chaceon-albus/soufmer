import { Terminal } from "lucide-react";
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

  return (
    <div className="overflow-hidden rounded-xl border border-slate-800 bg-slate-950 shadow-md">
      <div className="flex items-center gap-2 border-b border-slate-800/80 bg-slate-900/90 px-3.5 py-2 font-mono text-xs text-slate-300">
        <Terminal className="size-3.5 text-primary" />
        <span className="font-medium text-slate-300 tracking-wide">uv output</span>
      </div>
      <ScrollArea
        className="h-48 w-full font-mono text-xs bg-slate-950"
        viewportRef={viewportRef}
        onViewportScroll={(event) => {
          const viewport = event.currentTarget;
          followLatest.current = isUvTerminalAtTail(viewport.scrollHeight, viewport.scrollTop, viewport.clientHeight);
        }}
      >
        <div role="log" aria-label="uv output" aria-live="polite" className="min-h-full p-3 font-mono text-xs leading-5">
          {lines.length === 0 ? (
            <p className="font-mono text-slate-500 italic">{UV_TERMINAL_EMPTY_MESSAGE}</p>
          ) : (
            lines.map((line) => (
              <p key={line.sequence} data-level={line.level} className="break-all font-mono" style={{ color: UV_TERMINAL_COLORS[line.level] }}>
                {line.text}
              </p>
            ))
          )}
        </div>
      </ScrollArea>
    </div>
  );
}

