import { useEffect, useRef } from "react";
import { isDesktopBridge, setWindowContentHeight } from "@/lib/ipc";

interface AutoHeightOptions {
  enabled?: boolean;
  minHeight?: number;
  deps?: unknown[];
}

export function useWindowAutoHeight(
  containerRef: React.RefObject<HTMLElement | null>,
  options: AutoHeightOptions = {}
) {
  const { enabled = true, minHeight = 300, deps = [] } = options;
  const lastHeightRef = useRef<number>(0);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    if (!enabled || !isDesktopBridge() || !containerRef.current) return;

    const element = containerRef.current;

    const updateHeight = () => {
      if (!element) return;

      const rectHeight = element.getBoundingClientRect().height;
      if (rectHeight <= 0) return;
      const targetHeight = Math.max(minHeight, Math.ceil(rectHeight));

      if (Math.abs(lastHeightRef.current - targetHeight) <= 2) {
        return;
      }

      lastHeightRef.current = targetHeight;
      void setWindowContentHeight(targetHeight);
    };

    const handleResize = () => {
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
      }
      rafRef.current = requestAnimationFrame(() => {
        updateHeight();
      });
    };

    const observer = new ResizeObserver(() => {
      handleResize();
    });

    observer.observe(element);
    
    handleResize();
    const timer = setTimeout(handleResize, 50);

    return () => {
      observer.disconnect();
      clearTimeout(timer);
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [containerRef, enabled, minHeight, ...deps]);
}
