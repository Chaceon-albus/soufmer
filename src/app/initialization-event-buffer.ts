import type { AppAction } from "./app-reducer";

export type BufferedInitializationEvent = { taskId: string; sequence: number; action: AppAction; terminal: boolean };
export type InitializationEventBridge = { awaitingAcknowledgement: boolean; taskId?: string; pending: BufferedInitializationEvent[] };

export function bufferInitializationEvent(bridge: InitializationEventBridge, event: BufferedInitializationEvent) {
  if (bridge.awaitingAcknowledgement) {
    bridge.pending.push(event);
    return { handled: true as const };
  }
  if (bridge.taskId !== event.taskId) return { handled: false as const };
  return { handled: true as const, action: event.action, terminal: event.terminal };
}

export function acknowledgeInitializationEvents(bridge: InitializationEventBridge, taskId: string) {
  bridge.awaitingAcknowledgement = false;
  bridge.taskId = taskId;
  const events = bridge.pending
    .filter((event) => event.taskId === taskId)
    .sort((left, right) => left.sequence - right.sequence);
  bridge.pending = [];
  return { events, terminal: events.some((event) => event.terminal) };
}
