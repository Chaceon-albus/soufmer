import { cn } from "@/lib/utils";

export function Progress({ value, indeterminate = false }: { value?: number; indeterminate?: boolean }) {
  const width = `${Math.min(100, Math.max(0, value ?? 0))}%`;
  return <div className="h-2 overflow-hidden rounded-full bg-slate-200" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={indeterminate ? undefined : value}>
    <div className={cn("h-full bg-display-accent ring-1 ring-inset ring-primary transition-[width] duration-300", indeterminate && "w-1/3 animate-[progress_1.2s_ease-in-out_infinite]")} style={indeterminate ? undefined : { width }} />
  </div>;
}
