import { Check, Copy } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { getDiagnosticReport } from "@/lib/ipc";
import type { AppError } from "@/types/domain";

export function ErrorDialog({ error, onClose, onRetryInitialization }: { error: AppError; onClose: () => void; onRetryInitialization?: () => void }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const copyDiagnostics = async () => {
    const fallback = [error.code, error.diagnosticId, error.stage].join("\n");
    let report = fallback;
    try { report = await getDiagnosticReport(error.diagnosticId); } catch { /* The local summary remains useful when the persisted report is unavailable. */ }
    try {
      if (!navigator.clipboard?.writeText) return;
      await navigator.clipboard.writeText(report);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  return <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}><DialogContent aria-describedby="error-description"><DialogTitle className="text-lg font-semibold text-red-800">{t("error.title")}</DialogTitle><p id="error-description" className="mt-2 text-sm text-slate-700">{t(error.messageKey)}</p><dl className="mt-4 space-y-2 rounded-md bg-slate-100 p-3 text-xs"><div className="flex justify-between gap-3"><dt className="text-slate-600">{t("error.code")}</dt><dd className="break-all font-mono text-right">{error.code}</dd></div><div className="flex justify-between gap-3"><dt className="text-slate-600">{t("error.diagnosticId")}</dt><dd className="break-all font-mono text-right">{error.diagnosticId}</dd></div></dl><p className="mt-3 text-sm text-slate-600">{t("error.recovery")}</p><div className="mt-6 flex flex-col-reverse gap-3 sm:flex-row sm:justify-end"><Button type="button" variant="outline" className="w-full sm:w-auto" onClick={() => void copyDiagnostics()}>{copied ? <Check className="size-4" /> : <Copy className="size-4" />}{t(copied ? "error.copiedDiagnostics" : "error.copyDiagnostics")}</Button><Button type="button" variant={onRetryInitialization ? "outline" : "default"} className="w-full sm:w-auto" onClick={onClose}>{t("action.close")}</Button>{onRetryInitialization && <Button type="button" className="w-full sm:w-auto" onClick={onRetryInitialization}>{t("error.retryInitialization")}</Button>}</div></DialogContent></Dialog>;
}
