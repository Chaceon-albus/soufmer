import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";

type Props = {
  mode: "initializing" | "processing";
  onConfirm: () => void;
  onDecline: () => void;
};

export function CancellationConfirmationDialog({ mode, onConfirm, onDecline }: Props) {
  const { t } = useTranslation();
  const [confirming, setConfirming] = useState(false);
  const confirm = () => {
    if (confirming) return;
    setConfirming(true);
    onConfirm();
  };

  return <Dialog open onOpenChange={(open) => { if (!open && !confirming) onDecline(); }}>
    <DialogContent aria-describedby="cancellation-confirmation-description">
      <DialogTitle className="text-lg font-semibold">{t("cancellation.title")}</DialogTitle>
      <p id="cancellation-confirmation-description" className="mt-2 text-sm leading-6 text-slate-600">
        {t(mode === "initializing" ? "cancellation.initializationConsequence" : "cancellation.processingConsequence")}
      </p>
      <div className="mt-6 flex flex-col-reverse gap-3 sm:flex-row sm:justify-end">
        <Button type="button" variant="outline" className="w-full sm:w-auto" disabled={confirming} onClick={onDecline}>{t("cancellation.continue")}</Button>
        <Button type="button" variant="destructive" className="w-full sm:w-auto" disabled={confirming} onClick={confirm}>{t("cancellation.confirm")}</Button>
      </div>
    </DialogContent>
  </Dialog>;
}
