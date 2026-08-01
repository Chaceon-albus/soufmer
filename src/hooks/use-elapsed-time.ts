import { useEffect, useState } from "react";

export function useElapsedTime(active: boolean) {
  const [startedAt, setStartedAt] = useState<number>();
  const [now, setNow] = useState(Date.now());
  useEffect(() => { if (active) setStartedAt(Date.now()); else setStartedAt(undefined); }, [active]);
  useEffect(() => { if (!startedAt) return; const timer = window.setInterval(() => setNow(Date.now()), 1000); return () => window.clearInterval(timer); }, [startedAt]);
  return startedAt ? Math.floor((now - startedAt) / 1000) : 0;
}
