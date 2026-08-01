import { useTranslation } from "react-i18next";
import { attributions } from "@/lib/attributions";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";

export function LicenseDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  return <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}><DialogContent aria-describedby="license-description"><DialogTitle className="text-lg font-semibold">{t("license.title")}</DialogTitle><p id="license-description" className="mt-2 text-sm leading-6 text-slate-600">{t("license.description")}</p><ul className="mt-5 max-h-72 space-y-3 overflow-auto">{attributions.map((entry) => <li key={entry.name} className="rounded-lg border border-slate-200 p-3"><p className="font-medium">{entry.name}</p><p className="mt-2 text-sm text-slate-600">{t(entry.detailKey)}</p><p className="mt-1 text-xs font-medium text-slate-500">{t(entry.statusKey)}</p></li>)}</ul><div className="mt-6 flex justify-end"><Button type="button" className="w-full sm:w-auto" onClick={onClose}>{t("action.close")}</Button></div></DialogContent></Dialog>;
}
