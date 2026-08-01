import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { getLicenseNotices } from "@/lib/ipc";
import type { LicenseNotice } from "@/types/backend";

type NoticeState =
  | { type: "loading" }
  | { type: "ready"; notices: LicenseNotice[] }
  | { type: "failed" };

export function LicenseDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const [state, setState] = useState<NoticeState>({ type: "loading" });
  const [requestId, setRequestId] = useState(0);

  useEffect(() => {
    let active = true;
    setState({ type: "loading" });
    void getLicenseNotices().then(
      (notices) => { if (active) setState({ type: "ready", notices }); },
      () => { if (active) setState({ type: "failed" }); },
    );
    return () => { active = false; };
  }, [requestId]);

  return <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}><DialogContent aria-describedby="license-description"><DialogTitle className="text-lg font-semibold">{t("license.title")}</DialogTitle><p id="license-description" className="mt-2 text-sm leading-6 text-slate-600">{t("license.description")}</p>{state.type === "loading" && <p className="mt-5 text-sm text-slate-600" role="status">{t("license.loading")}</p>}{state.type === "failed" && <div className="mt-5 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-950" role="alert"><p>{t("license.loadFailed")}</p><Button type="button" variant="outline" size="sm" className="mt-3" onClick={() => setRequestId((value) => value + 1)}>{t("license.retry")}</Button></div>}{state.type === "ready" && <div className="mt-5 max-h-[50vh] space-y-4 overflow-y-auto pr-1">{state.notices.map((notice) => <section key={notice.id} className="rounded-lg border border-slate-200 p-3" aria-labelledby={`license-notice-${notice.id}`}><h2 id={`license-notice-${notice.id}`} className="font-medium">{notice.title}</h2><pre className="mt-3 whitespace-pre-wrap break-words font-mono text-xs leading-5 text-slate-700" tabIndex={0}>{notice.text}</pre></section>)}</div>}<div className="mt-6 flex justify-end"><Button type="button" className="w-full sm:w-auto" onClick={onClose}>{t("action.close")}</Button></div></DialogContent></Dialog>;
}
