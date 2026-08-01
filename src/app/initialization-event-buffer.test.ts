import { describe, expect, it } from "vitest";
import { acknowledgeInitializationEvents, bufferInitializationEvent, type InitializationEventBridge } from "./initialization-event-buffer";

describe("initialization event buffering", () => {
  it("replays pre-acknowledgement events in sequence and retains terminal state", () => {
    const bridge: InitializationEventBridge = { awaitingAcknowledgement: true, pending: [] };
    const error = { code: "PYTHON_SYNC_FAILED", stage: "runtime", messageKey: "error.pythonSyncFailed", recoverable: true, diagnosticId: "diagnostic-1" };
    bufferInitializationEvent(bridge, { taskId: "runtime-1", sequence: 3, terminal: true, action: { type: "eventFailed", taskId: "runtime-1", error } });
    bufferInitializationEvent(bridge, { taskId: "runtime-1", sequence: 1, terminal: false, action: { type: "initializationActivity", taskId: "runtime-1", sequence: 1, activity: { stepId: "installingPython", level: "status", message: "installingPython" } } });
    bufferInitializationEvent(bridge, { taskId: "other-task", sequence: 2, terminal: false, action: { type: "taskCancelled", taskId: "other-task" } });

    const replay = acknowledgeInitializationEvents(bridge, "runtime-1");

    expect(replay.events.map((event) => event.sequence)).toEqual([1, 3]);
    expect(replay.events.map((event) => event.action.type)).toEqual(["initializationActivity", "eventFailed"]);
    expect(replay.terminal).toBe(true);
  });

  it("dispatches only matching events after acknowledgement", () => {
    const bridge: InitializationEventBridge = { awaitingAcknowledgement: false, taskId: "runtime-1", pending: [] };
    const matching = bufferInitializationEvent(bridge, { taskId: "runtime-1", sequence: 1, terminal: false, action: { type: "taskCancelled", taskId: "runtime-1" } });
    const stale = bufferInitializationEvent(bridge, { taskId: "other-task", sequence: 2, terminal: false, action: { type: "taskCancelled", taskId: "other-task" } });

    expect(matching).toMatchObject({ handled: true, action: { type: "taskCancelled" } });
    expect(stale).toEqual({ handled: false });
  });
});
