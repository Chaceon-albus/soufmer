import { CircleAlert, CircleCheck, Download, Wrench } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { formatBytes } from "@/lib/format";
import type { EnvironmentStatus } from "@/types/domain";

export function EnvironmentStatusCard({ status, onInitialize, disabled = false }: { status: EnvironmentStatus; onInitialize: () => void; disabled?: boolean }) {
  const { t } = useTranslation();
  const ready = status.type === "ready";
  const action = status.type === "notInstalled" ? "initialize" : status.type === "repairRequired" ? "repair" : undefined;
  const detail = ready ? t("environment.readyDetail", status) : status.type === "notInstalled" ? t("environment.notInstalledDetail", { download: formatBytes(status.estimatedDownloadBytes), disk: formatBytes(status.estimatedDiskBytes) }) : status.type === "installing" ? t("environment.installingDetail") : status.type === "unsupported" ? t("environment.unsupportedDetail") : t("environment.repairDetail");

  return (
    <section className="flex min-w-0 items-center gap-3 rounded-xl border border-slate-200/90 bg-white/95 px-3.5 py-2.5 sm:px-4 sm:py-3 shadow-2xs backdrop-blur-xs transition-all" aria-labelledby="environment-status-title">
      <div className={`flex shrink-0 items-center justify-center rounded-lg p-1.5 ${ready ? "bg-emerald-50 text-emerald-600 border border-emerald-100" : "bg-amber-50 text-amber-600 border border-amber-100"}`}>
        {ready ? <CircleCheck className="size-4.5" /> : <CircleAlert className="size-4.5" />}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <p id="environment-status-title" className="text-xs font-semibold uppercase tracking-wider text-slate-500">{t("environment.title")}</p>
          <Badge className={`text-xs font-medium border ${ready ? "bg-emerald-50/80 text-emerald-700 border-emerald-200/70" : "bg-amber-50/80 text-amber-700 border-amber-200/70"}`}>
            {t(`environment.status.${status.type}`)}
          </Badge>
        </div>
        <p className="mt-0.5 text-xs sm:text-sm leading-relaxed text-slate-600">{detail}</p>
      </div>
      {action && (
        <Button type="button" size="sm" variant="outline" className="shrink-0 font-medium" disabled={disabled} onClick={onInitialize}>
          {action === "repair" ? <Wrench className="size-3.5 mr-1" /> : <Download className="size-3.5 mr-1" />}
          {t(`environment.${action}`)}
        </Button>
      )}
    </section>
  );
}

