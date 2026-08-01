import { useEffect, useRef } from "react";
import type { TFunction } from "i18next";
import { LoaderCircle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatBytes, formatDuration, formatRate } from "@/lib/format";
import { useElapsedTime } from "@/hooks/use-elapsed-time";
import type { BatchProgress, InitializationActivity, InitializationActivityEntry, InitializationProgress, ProgressValue } from "@/types/domain";

type Props = {
  progress: InitializationProgress | BatchProgress;
  activities?: InitializationActivityEntry[];
  mode: "initializing" | "processing" | "cancelling";
  onCancel: () => void;
};

const isInitialization = (value: InitializationProgress | BatchProgress): value is InitializationProgress => "stepIndex" in value;

function ProgressRow({ label, value }: { label: string; value: ProgressValue }) {
  return <div>
    <div className="mb-2 flex gap-3 text-sm text-slate-600">
      <span className="min-w-0 flex-1">{label}</span>
      {value.kind === "determinate" && <span className="shrink-0">{Math.round((value.fraction ?? 0) * 100)}%</span>}
    </div>
    <Progress value={(value.fraction ?? 0) * 100} indeterminate={value.kind === "indeterminate"} />
  </div>;
}

export function TaskProgressDialog({ progress, activities = [], mode, onCancel }: Props) {
  const { t } = useTranslation();
  const initializing = isInitialization(progress);
  const stage = initializing ? t(`progress.initialization.${progress.stepId}`) : t(`progress.stage.${progress.stage}`);
  const currentItem = initializing ? stage : progress.currentDisplayName;
  const count = initializing ? t("progress.stepCount", { current: progress.stepIndex, total: progress.stepCount }) : t("progress.itemCount", { current: progress.itemIndex, total: progress.itemCount });
  const localElapsed = useElapsedTime(mode !== "cancelling");
  const elapsed = initializing ? localElapsed : progress.elapsedSeconds;
  const downloadVisible = initializing && progress.bytesCompleted !== undefined && progress.bytesTotal !== undefined;
  const currentActivity = initializing ? findCurrentActivity(activities, progress.stepId) : undefined;
  const activityText = currentActivity ? formatActivity(currentActivity, t) : initializing ? t(progress.stepId === "syncingEnvironment" ? "progress.activity.syncingCudaEnvironment" : "progress.activity.working") : undefined;

  return <Dialog open>
    <DialogContent aria-describedby="task-progress-description">
      <div className="flex items-start gap-3">
        <LoaderCircle className="mt-1 size-5 shrink-0 animate-spin text-primary" />
        <div className="min-w-0">
          <DialogTitle className="text-lg font-semibold">{t(mode === "initializing" ? "progress.initializingTitle" : mode === "cancelling" ? "progress.cancellingTitle" : "progress.processingTitle")}</DialogTitle>
          <p id="task-progress-description" className="mt-1 text-sm text-slate-600">{count}</p>
        </div>
      </div>
      <div className="mt-6 space-y-5">
        <ProgressRow label={t("progress.overall")} value={progress.overall} />
        <div>
          <p className="truncate text-sm font-medium text-slate-800">{currentItem || t("progress.waiting")}</p>
          {!initializing && <p className="mt-1 text-sm text-slate-600">{stage}</p>}
          {activityText && <p aria-live="polite" className="mt-1 text-sm leading-5 text-slate-600">{activityText}</p>}
        </div>
        <ProgressRow label={t("progress.currentTask")} value={progress.current} />
        {downloadVisible && <dl className="grid gap-3 rounded-lg bg-slate-50 p-3 text-sm sm:grid-cols-2">
          <div><dt className="text-slate-600">{t("progress.downloaded")}</dt><dd className="mt-1 break-words font-medium">{formatBytes(progress.bytesCompleted)} / {formatBytes(progress.bytesTotal)}</dd></div>
          <div><dt className="text-slate-600">{t("progress.speed")}</dt><dd className="mt-1 font-medium">{formatRate(progress.bytesPerSecond)}</dd></div>
        </dl>}
        {initializing && activities.length > 0 && <ActivityFeed activities={activities} />}
        <p className="text-sm text-slate-600">{t("progress.elapsed", { time: formatDuration(elapsed) })}</p>
      </div>
      <div className="mt-7 flex justify-end">
        <Button type="button" variant="outline" disabled={mode === "cancelling"} onClick={onCancel}>{t(mode === "cancelling" ? "progress.cancelling" : "action.cancel")}</Button>
      </div>
    </DialogContent>
  </Dialog>;
}

function ActivityFeed({ activities }: { activities: InitializationActivityEntry[] }) {
  const { t } = useTranslation();
  const viewportRef = useRef<HTMLDivElement>(null);
  const followLatest = useRef(true);
  const latestSequence = activities.at(-1)?.sequence;

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || !followLatest.current) return;
    const frame = requestAnimationFrame(() => viewport.scrollTo({ top: viewport.scrollHeight }));
    return () => cancelAnimationFrame(frame);
  }, [latestSequence]);

  return <div>
    <p className="mb-2 text-xs font-medium uppercase text-slate-500">{t("progress.recentActivity")}</p>
    <ScrollArea
      className="h-28 rounded-lg border border-slate-200 bg-slate-50"
      viewportRef={viewportRef}
      onViewportScroll={(event) => {
        const viewport = event.currentTarget;
        followLatest.current = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < 8;
      }}
    >
      <ol className="space-y-1.5 p-3" aria-label={t("progress.recentActivity")}>
        {activities.map(({ sequence, activity }) => <li key={sequence} className="flex gap-2 text-xs leading-5 text-slate-600">
          <span aria-hidden className="mt-2 h-1.5 w-1.5 shrink-0 rounded-sm bg-display-accent ring-1 ring-primary" />
          <span className="min-w-0 break-words">{formatActivity(activity, t)}</span>
        </li>)}
      </ol>
    </ScrollArea>
  </div>;
}

function findCurrentActivity(activities: InitializationActivityEntry[], stepId: string) {
  for (let index = activities.length - 1; index >= 0; index -= 1) {
    if (activities[index].activity.stepId === stepId) return activities[index].activity;
  }
  return undefined;
}

function formatActivity(activity: InitializationActivity, t: TFunction) {
  return t(`progress.activity.${activity.message}`, {
    packageName: activity.packageName,
    count: activity.completedUnits,
  });
}
