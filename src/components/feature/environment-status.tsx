import { CircleAlert, CircleCheck, Download, Wrench } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { formatBytes } from "@/lib/format";
import type { EnvironmentStatus } from "@/types/domain";

export function EnvironmentStatusCard({ status, onInitialize, disabled = false }: { status: EnvironmentStatus; onInitialize: () => void; disabled?: boolean }) {
  const { t } = useTranslation();
  const ready = status.type === "ready";
  const action = status.type === "notInstalled" ? "initialize" : status.type === "repairRequired" ? "repair" : undefined;
  const detail = ready ? t("environment.readyDetail", status) : status.type === "notInstalled" ? t("environment.notInstalledDetail", { download: formatBytes(status.estimatedDownloadBytes), disk: formatBytes(status.estimatedDiskBytes) }) : status.type === "installing" ? t("environment.installingDetail") : status.type === "unsupported" ? t("environment.unsupportedDetail") : t("environment.repairDetail");
  return <Card><CardContent className="flex flex-col gap-3 py-4 sm:flex-row sm:items-center"><div className={ready ? "text-emerald-600" : "text-amber-600"}>{ready ? <CircleCheck /> : <CircleAlert />}</div><div className="min-w-0 flex-1"><div className="flex flex-wrap items-center gap-2"><p className="font-medium">{t("environment.title")}</p><Badge className={ready ? "bg-emerald-100 text-emerald-800" : "bg-amber-100 text-amber-800"}>{t(`environment.status.${status.type}`)}</Badge></div><p className="mt-1 text-sm text-slate-600">{detail}</p></div>{action && <Button type="button" variant="outline" className="w-full sm:w-auto" disabled={disabled} onClick={onInitialize}>{action === "repair" ? <Wrench className="size-4" /> : <Download className="size-4" />}{t(`environment.${action}`)}</Button>}</CardContent></Card>;
}
