import { useCallback, useEffect, useRef } from "react";
import { saveAppSettings } from "@/lib/ipc";
import type { AppSettings } from "@/types/domain";

export function useSettingsPersistence() {
  const timer = useRef<number | undefined>(undefined);
  const queue = useRef(Promise.resolve());
  const pendingSettings = useRef<AppSettings | undefined>(undefined);

  const flush = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = undefined;

    const settings = pendingSettings.current;
    pendingSettings.current = undefined;
    if (settings) {
      queue.current = queue.current.catch(() => undefined).then(() => saveAppSettings(settings));
    }
  }, []);

  const schedule = useCallback((settings: AppSettings) => {
    pendingSettings.current = settings;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      flush();
    }, 250);
  }, [flush]);

  useEffect(() => flush, [flush]);

  return { schedule, flush };
}
