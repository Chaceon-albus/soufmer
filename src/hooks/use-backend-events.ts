import { useEffect, useState } from "react";
import { batchItemSchema, batchProgressSchema, backendErrorSchema, backendEventSchema, batchResultSchema, initializationProgressSchema, readyEnvironmentSchema } from "@/types/backend";
import { eventNames, subscribe, toAppError, toBatchResult } from "@/lib/ipc";
import type { BackendEventName } from "@/lib/ipc";

type Handlers = {
  onInitializationProgress: (event: { taskId: string; sequence: number; progress: ReturnType<typeof initializationProgressSchema.parse> }) => void;
  onInitializationCompleted: (event: { taskId: string; sequence: number; environment: ReturnType<typeof readyEnvironmentSchema.parse> }) => void;
  onBatchProgress: (event: { taskId: string; sequence: number; progress: ReturnType<typeof batchProgressSchema.parse> }) => void;
  onItemCompleted: (event: { taskId: string; sequence: number }) => void;
  onCompleted: (event: { taskId: string; result: ReturnType<typeof toBatchResult> }) => void;
  onFailed: (event: { taskId: string; error: ReturnType<typeof toAppError> }) => void;
  onCancelled: (taskId: string) => void;
};
const schemas = {
  "runtime://progress": backendEventSchema("runtime://progress", initializationProgressSchema),
  "runtime://completed": backendEventSchema("runtime://completed", readyEnvironmentSchema),
  "batch://progress": backendEventSchema("batch://progress", batchProgressSchema),
  "batch://item-completed": backendEventSchema("batch://item-completed", batchItemSchema),
  "batch://completed": backendEventSchema("batch://completed", batchResultSchema),
  "task://failed": backendEventSchema("task://failed", backendErrorSchema),
  "task://cancelled": backendEventSchema("task://cancelled", backendErrorSchema),
} as const;

export function useBackendEvents(handlers: Handlers) {
  const [listenersReady, setListenersReady] = useState(false);

  useEffect(() => {
    let active = true;
    const unlisten: Array<() => void> = [];
    const subscribeAll = async () => {
      try {
        await Promise.all(eventNames.map(async (name) => {
          const stop = await subscribe(name, (payload) => {
            if (!active) return;
            const parsed = schemas[name].safeParse(payload);
            if (!parsed.success) return;
            routeEvent(name, parsed.data, handlers);
          });
          if (active) unlisten.push(stop); else stop();
        }));
        if (active) setListenersReady(true);
      } catch {
        if (active) setListenersReady(false);
      }
    };
    setListenersReady(false);
    void subscribeAll();
    return () => { active = false; setListenersReady(false); unlisten.forEach((stop) => stop()); };
  }, [handlers]);

  return listenersReady;
}

function routeEvent(name: BackendEventName, event: { taskId: string; sequence: number; payload: unknown }, handlers: Handlers) {
  if (name === "runtime://progress") handlers.onInitializationProgress({ taskId: event.taskId, sequence: event.sequence, progress: initializationProgressSchema.parse(event.payload) });
  if (name === "runtime://completed") handlers.onInitializationCompleted({ taskId: event.taskId, sequence: event.sequence, environment: readyEnvironmentSchema.parse(event.payload) });
  if (name === "batch://progress") handlers.onBatchProgress({ taskId: event.taskId, sequence: event.sequence, progress: batchProgressSchema.parse(event.payload) });
  if (name === "batch://item-completed") handlers.onItemCompleted({ taskId: event.taskId, sequence: event.sequence });
  if (name === "batch://completed") handlers.onCompleted({ taskId: event.taskId, result: toBatchResult(event.payload) });
  if (name === "task://failed") handlers.onFailed({ taskId: event.taskId, error: toAppError(event.payload) });
  if (name === "task://cancelled") handlers.onCancelled(event.taskId);
}
