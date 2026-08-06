import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { CancellationConfirmationDialog } from "@/components/feature/cancellation-confirmation-dialog";
import { useTranslation } from "react-i18next";
import { CompletionDialog } from "@/components/feature/completion-dialog";
import { EnvironmentStatusCard } from "@/components/feature/environment-status";
import { ErrorDialog } from "@/components/feature/error-dialog";
import { LicenseDialog } from "@/components/feature/license-dialog";
import { MainForm } from "@/components/feature/main-form";
import { TaskProgressDialog } from "@/components/feature/task-progress-dialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { useBackendEvents } from "@/hooks/use-backend-events";
import { useSettingsPersistence } from "@/hooks/use-settings-persistence";
import { useWindowAutoHeight } from "@/hooks/use-window-auto-height";
import { formatBytes } from "@/lib/format";
import { cancelActiveTask, chooseFolder, chooseInputFile, chooseOutputDirectory, getAppSettings, getEnvironmentStatus, initializeEnvironment, isDesktopBridge, revealOutputDirectory, startBatch, toAppError } from "@/lib/ipc";
import type { BatchProgress, EnvironmentStatus, InitializationActivity, InitializationProgress, StartBatchRequest } from "@/types/domain";
import { appReducer } from "./app-reducer";
import { defaultSettings, mockEnvironment, type AppState } from "./app-state";
import { acknowledgeInitializationEvents, bufferInitializationEvent, type BufferedInitializationEvent, type InitializationEventBridge } from "./initialization-event-buffer";

const initializationPlaceholder: InitializationProgress = { runtimeVersion: "", stepIndex: 0, stepCount: 1, stepId: "checkingSystem", overall: { kind: "indeterminate" }, current: { kind: "indeterminate" } };
const batchPlaceholder: BatchProgress = { itemIndex: 0, itemCount: 1, currentInputPath: "", currentDisplayName: "", stage: "probing", overall: { kind: "indeterminate" }, current: { kind: "indeterminate" }, completedDurationSeconds: 0, totalDurationSeconds: 0, elapsedSeconds: 0 };
const mainFormId = "main-batch-form";

export default function App() {
  const { t, i18n } = useTranslation();
  const [state, dispatch] = useReducer(appReducer, { type: "booting" } as AppState);
  const contentRef = useRef<HTMLDivElement>(null);
  const currentEnvironment = state.type === "booting" ? mockEnvironment : state.environment ?? mockEnvironment;
  useWindowAutoHeight(contentRef, { deps: [i18n.language, state.type, currentEnvironment.type] });
  const [licenseOpen, setLicenseOpen] = useState(false);
  const [cancellationConfirmation, setCancellationConfirmation] = useState<{ taskId: string; mode: "initializing" | "processing" }>();
  const cancellationStartedForTask = useRef<string | undefined>(undefined);
  const initializationEventBridge = useRef<InitializationEventBridge | undefined>(undefined);
  const { schedule: queueSettingsSave } = useSettingsPersistence();
  const dispatchInitializationEvent = useCallback((event: BufferedInitializationEvent) => {
    const bridge = initializationEventBridge.current;
    if (!bridge) return false;
    const result = bufferInitializationEvent(bridge, event);
    if (!result.handled) return false;
    if (result.action) dispatch(result.action);
    if (result.terminal) initializationEventBridge.current = undefined;
    return result.handled;
  }, []);
  const handlers = useMemo(() => ({
    onInitializationProgress: (event: { taskId: string; sequence: number; progress: InitializationProgress }) => { dispatchInitializationEvent({ taskId: event.taskId, sequence: event.sequence, action: { type: "initializationProgress", ...event }, terminal: false }); },
    onInitializationActivity: (event: { taskId: string; sequence: number; activity: InitializationActivity }) => { dispatchInitializationEvent({ taskId: event.taskId, sequence: event.sequence, action: { type: "initializationActivity", ...event }, terminal: false }); },
    onInitializationCompleted: (event: { taskId: string; sequence: number; environment: Extract<import("@/types/domain").EnvironmentStatus, { type: "ready" }> }) => { dispatchInitializationEvent({ taskId: event.taskId, sequence: event.sequence, action: { type: "initializationCompleted", ...event }, terminal: true }); },
    onBatchProgress: (event: { taskId: string; sequence: number; progress: BatchProgress }) => dispatch({ type: "batchProgress", ...event }),
    onItemCompleted: () => undefined,
    onCompleted: (event: { taskId: string; result: import("@/types/domain").BatchResult }) => dispatch({ type: "eventCompleted", ...event }),
    onFailed: (event: { taskId: string; sequence: number; error: import("@/types/domain").AppError }) => { if (dispatchInitializationEvent({ taskId: event.taskId, sequence: event.sequence, action: { type: "eventFailed", taskId: event.taskId, error: event.error }, terminal: true })) return; if (event.error.code === "ENV_NOT_INITIALIZED") { void getEnvironmentStatus().then((environment) => dispatch({ type: "environmentNotReady", environment })); return; } dispatch({ type: "eventFailed", taskId: event.taskId, error: event.error }); },
    onCancelled: (event: { taskId: string; sequence: number }) => { if (!dispatchInitializationEvent({ taskId: event.taskId, sequence: event.sequence, action: { type: "taskCancelled", taskId: event.taskId }, terminal: true })) dispatch({ type: "taskCancelled", taskId: event.taskId }); },
  }), [dispatchInitializationEvent]);
  const listenersReady = useBackendEvents(handlers);

  useEffect(() => { void Promise.all([getEnvironmentStatus(), getAppSettings()]).then(([environment, settings]) => { void i18n.changeLanguage(settings.locale); dispatch({ type: "booted", environment, settings }); }).catch((error: unknown) => dispatch({ type: "failed", error: toAppError(error) })); }, [i18n]);
  useEffect(() => {
    const preventDefault = (e: DragEvent) => { e.preventDefault(); };
    window.addEventListener("dragover", preventDefault);
    window.addEventListener("drop", preventDefault);
    return () => {
      window.removeEventListener("dragover", preventDefault);
      window.removeEventListener("drop", preventDefault);
    };
  }, []);
  useEffect(() => { if (state.type !== "validating") return; void getEnvironmentStatus().then((environment) => { if (environment.type !== "ready") { dispatch({ type: "validationNeedsInitialization", environment }); return; } return startBatch(state.request).then((acknowledgement) => dispatch({ type: "validationPassed", taskId: acknowledgement.taskId, progress: batchPlaceholder })); }).catch((error: unknown) => { const appError = toAppError(error); if (appError.code === "ENV_NOT_INITIALIZED") { void getEnvironmentStatus().then((environment) => dispatch({ type: "environmentNotReady", environment })); return; } dispatch({ type: "failed", error: appError }); }); }, [state]);

  const requestInitialization = useCallback(() => { if (listenersReady) dispatch({ type: "initializationRequested" }); }, [listenersReady]);
  const beginInitialization = useCallback(() => {
    if (!listenersReady || state.type !== "awaitingInitializationConsent" || initializationEventBridge.current) return;
    initializationEventBridge.current = { awaitingAcknowledgement: true, pending: [] };
    void initializeEnvironment().then((acknowledgement) => {
      const bridge = initializationEventBridge.current;
      if (!bridge) return;
      dispatch({ type: "initializationAccepted", taskId: acknowledgement.taskId, progress: initializationPlaceholder });
      const replay = acknowledgeInitializationEvents(bridge, acknowledgement.taskId);
      for (const event of replay.events) dispatch(event.action);
      if (replay.terminal) initializationEventBridge.current = undefined;
    }).catch((error: unknown) => {
      initializationEventBridge.current = undefined;
      dispatch({ type: "failed", error: toAppError(error) });
    });
  }, [listenersReady, state]);
  const requestCancellation = useCallback(() => { if ((state.type !== "initializing" && state.type !== "processing") || cancellationConfirmation || cancellationStartedForTask.current === state.taskId) return; setCancellationConfirmation({ taskId: state.taskId, mode: state.type }); }, [cancellationConfirmation, state]);
  const declineCancellation = useCallback(() => setCancellationConfirmation(undefined), []);
  const confirmCancellation = useCallback(() => {
    const confirmation = cancellationConfirmation;
    if (!confirmation || cancellationStartedForTask.current === confirmation.taskId || (state.type !== "initializing" && state.type !== "processing") || state.taskId !== confirmation.taskId) return;
    cancellationStartedForTask.current = confirmation.taskId;
    setCancellationConfirmation(undefined);
    dispatch({ type: "cancelRequested" });
    void cancelActiveTask(confirmation.taskId).catch((error: unknown) => dispatch({ type: "eventFailed", taskId: confirmation.taskId, error: toAppError(error) }));
  }, [cancellationConfirmation, state]);
  const dismiss = useCallback(() => { const context = state.type === "booting" ? { environment: mockEnvironment, settings: defaultSettings } : { environment: state.environment ?? mockEnvironment, settings: state.settings ?? defaultSettings }; dispatch({ type: "dismissed", ...context }); }, [state]);
  const retryInitialization = useCallback(() => dispatch({ type: "initializationRetryRequested" }), []);
  const chooseInput = useCallback(async (mode: "file" | "folder") => mode === "file" ? chooseInputFile() : chooseFolder(), []);
  const persistSettings = useCallback((settings: import("@/types/domain").AppSettings) => { dispatch({ type: "settingsUpdated", settings }); queueSettingsSave(settings); }, [queueSettingsSave]);
  const switchLanguage = useCallback(() => { const locale = i18n.language === "zh-CN" ? "en" : "zh-CN"; void i18n.changeLanguage(locale); if (state.type !== "booting" && state.settings) persistSettings({ ...state.settings, locale }); }, [i18n, persistSettings, state]);

  const currentSettings = state.type === "booting" ? defaultSettings : state.settings ?? defaultSettings;
  const isBusy = state.type === "initializing" || state.type === "processing" || state.type === "cancelling" || state.type === "validating";

  return <main className="w-full bg-slate-50 text-slate-900">
    <div ref={contentRef} className="mx-auto flex w-full max-w-4xl flex-col px-4 py-4 sm:px-6 space-y-3.5">
      <header className="flex items-center justify-between gap-4 border-b border-slate-200 pb-3 pr-2 shrink-0">
        <div className="flex items-center gap-2.5 min-w-0">
          <span aria-hidden className="h-4.5 w-1.5 shrink-0 rounded-full bg-gradient-to-b from-primary to-display-accent shadow-xs shadow-pink-300" />
          <h1 className="text-sm font-medium text-slate-700">{t("app.description")}</h1>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Button type="button" variant="ghost" size="sm" onClick={() => setLicenseOpen(true)}>{t("license.open")}</Button>
          <Button type="button" variant="ghost" size="sm" onClick={switchLanguage}>{t("app.switchLanguage")}</Button>
        </div>
      </header>
      <Card><CardContent className="p-4"><MainForm formId={mainFormId} settings={currentSettings} onSubmit={(request: StartBatchRequest) => dispatch({ type: "startRequested", request })} onSettingsChange={persistSettings} onChooseInput={chooseInput} onChooseOutput={chooseOutputDirectory} submitDisabled={state.type === "booting" || !listenersReady || currentEnvironment.type !== "ready" || isBusy} /></CardContent></Card>
      <EnvironmentStatusCard status={currentEnvironment} onInitialize={requestInitialization} disabled={state.type === "booting" || !listenersReady || isBusy} />
      {import.meta.env.DEV && !isDesktopBridge() && <div className="mt-3 space-y-2"><p className="text-center text-xs text-slate-500">{t("development.browserFallback")}</p><div className="flex flex-wrap justify-center gap-2"><Button type="button" size="sm" variant="outline" onClick={() => dispatch({ type: "developmentCompleted", result: { taskId: "browser-cancelled", succeeded: 0, failed: 0, skipped: 0, outputDirectory: "", cancelled: true, items: [] } })}>{t("development.previewCancelled")}</Button><Button type="button" size="sm" variant="outline" onClick={() => dispatch({ type: "failed", error: { code: "ENV_NOT_INITIALIZED", stage: "runtime", messageKey: "error.environmentNotInitialized", recoverable: true, diagnosticId: "browser-preview" } })}>{t("development.previewError")}</Button></div></div>}
    </div>
    {licenseOpen && <LicenseDialog onClose={() => setLicenseOpen(false)} />}
    {state.type === "awaitingInitializationConsent" && <InitializationConsent environment={state.environment} onAccept={beginInitialization} onDecline={dismiss} />}
    {cancellationConfirmation && <CancellationConfirmationDialog mode={cancellationConfirmation.mode} onConfirm={confirmCancellation} onDecline={declineCancellation} />}
    {state.type === "initializing" && <TaskProgressDialog progress={state.progress} activities={state.activities} mode="initializing" onCancel={requestCancellation} />}
    {state.type === "processing" && <TaskProgressDialog progress={state.progress} mode="processing" onCancel={requestCancellation} />}
    {state.type === "cancelling" && state.lastProgress && <TaskProgressDialog progress={state.lastProgress} activities={state.initializationActivities} mode="cancelling" onCancel={() => undefined} />}
    {state.type === "completed" && <CompletionDialog result={state.result} onDone={dismiss} onOpenOutput={() => void revealOutputDirectory(state.result.outputDirectory)} />}
    {state.type === "failed" && <ErrorDialog error={state.error} onClose={dismiss} onRetryInitialization={state.error.recoverable && state.initializationRequest ? retryInitialization : undefined} />}
  </main>;
}

function InitializationConsent({ environment, onAccept, onDecline }: { environment: EnvironmentStatus; onAccept: () => void; onDecline: () => void }) {
  const { t } = useTranslation();
  const estimates = environment.type === "notInstalled" || environment.type === "repairRequired" ? environment : undefined;
  return <Dialog open onOpenChange={(open) => { if (!open) onDecline(); }}><DialogContent aria-describedby="initialization-description"><DialogTitle className="text-lg font-semibold">{t("initialization.title")}</DialogTitle><p id="initialization-description" className="mt-2 text-sm leading-6 text-slate-600">{t("initialization.description")}</p><dl className="mt-4 grid gap-3 rounded-lg bg-slate-50 p-4 text-sm sm:grid-cols-2"><div><dt className="text-slate-600">{t("initialization.estimatedDownload")}</dt><dd className="mt-1 font-medium text-slate-900">{formatBytes(estimates?.estimatedDownloadBytes)}</dd></div><div><dt className="text-slate-600">{t("initialization.estimatedDisk")}</dt><dd className="mt-1 font-medium text-slate-900">{formatBytes(estimates?.estimatedDiskBytes)}</dd></div></dl><div className="mt-6 flex flex-col-reverse gap-3 sm:flex-row sm:justify-end"><Button type="button" variant="outline" className="w-full sm:w-auto" onClick={onDecline}>{t("action.cancel")}</Button><Button type="button" className="w-full sm:w-auto" onClick={onAccept}>{t("initialization.install")}</Button></div></DialogContent></Dialog>;
}
