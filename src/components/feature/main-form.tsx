import { Check, FileAudio, Folder, FolderOpen, SlidersHorizontal, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { startBatchRequestSchema } from "@/lib/schemas";
import { cn } from "@/lib/utils";
import type { AppSettings, InputMode, StartBatchRequest } from "@/types/domain";

type Props = { formId: string; settings: AppSettings; onSubmit: (request: StartBatchRequest) => void; onSettingsChange: (settings: AppSettings) => void; onChooseInput: (mode: InputMode) => Promise<string | null>; onChooseOutput: () => Promise<string | null>; submitDisabled?: boolean };
export function MainForm({ formId, settings, onSubmit, onSettingsChange, onChooseInput, onChooseOutput, submitDisabled = false }: Props) {
  const { t } = useTranslation();
  const [inputMode, setInputMode] = useState<InputMode>(settings.lastInputMode);
  const [inputPath, setInputPath] = useState("");
  const [outputDirectory, setOutputDirectory] = useState(settings.lastOutputDirectory ?? "");
  const [processingMode, setProcessingMode] = useState(settings.processingMode);
  const [recursive, setRecursive] = useState(settings.recursive);
  const [preserveDirectoryStructure, setPreserveDirectoryStructure] = useState(settings.preserveDirectoryStructure);
  const [generateBothModes, setGenerateBothModes] = useState(settings.generateBothModes);
  const [conflictPolicy, setConflictPolicy] = useState(settings.conflictPolicy);
  const [outputFormat, setOutputFormat] = useState(settings.outputFormat);
  const [error, setError] = useState<{ fieldId: string; message: string }>();
  useEffect(() => { onSettingsChange({ ...settings, lastInputMode: inputMode, lastOutputDirectory: outputDirectory || undefined, processingMode, recursive, preserveDirectoryStructure, generateBothModes, conflictPolicy, outputFormat }); }, [conflictPolicy, generateBothModes, inputMode, onSettingsChange, outputDirectory, outputFormat, preserveDirectoryStructure, processingMode, recursive, settings]);
  const chooseLabel = inputMode === "file" ? t("main.input.chooseFile") : t("main.input.chooseFolder");
  const submit = () => {
    if (submitDisabled) return;
    const parsed = startBatchRequestSchema.safeParse({ inputMode, inputPath, outputDirectory, processingMode, generateBothModes, recursive, preserveDirectoryStructure, conflictPolicy, outputFormat });
    if (!parsed.success) {
      const issue = parsed.error.issues[0];
      const invalidId = issue?.path[0] === "outputDirectory" ? "output-directory" : "input-path";
      setError({ fieldId: invalidId, message: issue?.message ?? "validation.inputRequired" });
      requestAnimationFrame(() => document.getElementById(invalidId)?.focus());
      return;
    }
    setError(undefined); onSubmit(parsed.data);
  };
  return <form id={formId} className="space-y-4" onSubmit={(event) => { event.preventDefault(); submit(); }}>
    <fieldset><legend className="mb-2 text-sm font-medium text-slate-700">{t("main.input.type")}</legend><div className="relative grid grid-cols-2 rounded-xl bg-pink-50/60 p-1 ring-1 ring-pink-100"><div className={cn("absolute top-1 bottom-1 w-[calc(50%-4px)] rounded-lg bg-selected shadow-xs ring-1 ring-inset ring-primary transition-all duration-200 ease-out", inputMode === "file" ? "left-1" : "left-[calc(50%+2px)]")} />{(["file", "folder"] as const).map((mode) => <label key={mode} className={cn("relative z-10 flex min-h-10 cursor-pointer items-center justify-center gap-2 rounded-lg text-sm font-medium transition-colors duration-200 select-none", inputMode === mode ? "text-slate-900" : "text-slate-600 hover:text-slate-900")}><input className="sr-only" type="radio" name="input-mode" value={mode} checked={inputMode === mode} onChange={() => setInputMode(mode)} /><span className={inputMode === mode ? "text-primary" : undefined}>{mode === "file" ? <FileAudio className="size-4" /> : <Folder className="size-4" />}</span>{t(`main.input.${mode}`)}</label>)}</div></fieldset>
    <PathField id="input-path" label={t("main.input.path")} value={inputPath} onChange={setInputPath} placeholder={t("main.input.placeholder")} actionLabel={chooseLabel} invalid={error?.fieldId === "input-path"} onBrowse={async () => { const path = await onChooseInput(inputMode); if (path) setInputPath(path); }} />
    <PathField id="output-directory" label={t("main.output.label")} value={outputDirectory} onChange={setOutputDirectory} placeholder={t("main.output.placeholder")} actionLabel={t("main.output.choose")} invalid={error?.fieldId === "output-directory"} onBrowse={async () => { const path = await onChooseOutput(); if (path) setOutputDirectory(path); }} />
    <fieldset><legend className="mb-2 text-sm font-medium text-slate-700">{t("main.mode.label")}</legend><div className="grid gap-3 sm:grid-cols-2">{(["compatibility44100", "sourceSampleRate"] as const).map((mode) => <label key={mode} className={cn("cursor-pointer rounded-xl border p-3.5 transition-all duration-200 ease-out select-none", processingMode === mode ? "border-primary bg-selected/80 shadow-xs shadow-pink-100/60 ring-1 ring-primary/20" : "border-slate-200/90 hover:border-pink-200/80 active:bg-slate-50")}><input className="sr-only" type="radio" name="processing-mode" value={mode} checked={processingMode === mode} onChange={() => setProcessingMode(mode)} /><span className="flex items-start gap-3"><span className={cn("mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border-2 transition-all duration-200 ease-out", processingMode === mode ? "border-primary bg-primary ring-4 ring-pink-100/80" : "border-slate-300 bg-white")}>{processingMode === mode && <span className="size-1.5 rounded-full bg-white" />}</span><span><span className="block font-medium text-slate-900">{t(`main.mode.${mode}.title`)}</span><span className="mt-1 block text-sm leading-5 text-slate-600">{t(`main.mode.${mode}.description`)}</span></span></span></label>)}</div></fieldset>
    <div className="flex items-center gap-3 pt-1">
      <Button type="submit" size="lg" className="h-11 flex-1 text-base font-semibold shadow-2xs hover:shadow-xs transition-all active:scale-[0.995]" disabled={submitDisabled}>
        <Sparkles className="size-4.5" />
        {t("main.start")}
      </Button>
      <Popover>
        <PopoverTrigger asChild>
          <Button type="button" variant="outline" className="h-11 shrink-0 gap-2 border-slate-200/90 hover:border-pink-200">
            <SlidersHorizontal className="size-4 text-primary" />
            <span>{t("advanced.title")}</span>
          </Button>
        </PopoverTrigger>
        <PopoverContent align="end" className="w-80 p-4 shadow-lg border-slate-200">
          <div className="space-y-4">
            <p className="font-semibold text-sm text-slate-900 border-b border-slate-100 pb-2">{t("advanced.title")}</p>
            {inputMode === "folder" && (
              <>
                <CheckOption checked={recursive} onChange={setRecursive} label={t("advanced.recursive")} />
                <CheckOption checked={preserveDirectoryStructure} onChange={setPreserveDirectoryStructure} label={t("advanced.preserveStructure")} />
              </>
            )}
            <CheckOption checked={generateBothModes} onChange={setGenerateBothModes} label={t("advanced.generateBoth")} />
            <SelectOption label={t("advanced.outputFormat")} value={outputFormat} onChange={(value) => setOutputFormat(value as typeof outputFormat)} options={[["flac", t("advanced.formatFlac")], ["wavFloat32", t("advanced.formatWav")]]} />
            <SelectOption label={t("advanced.conflictPolicy")} value={conflictPolicy} onChange={(value) => setConflictPolicy(value as typeof conflictPolicy)} options={[["skip", t("advanced.conflictSkip")], ["overwrite", t("advanced.conflictOverwrite")], ["autoNumber", t("advanced.conflictAutoNumber")]]} />
          </div>
        </PopoverContent>
      </Popover>
    </div>
    {error && <p id="main-form-error" className="text-sm text-red-700" role="alert">{t(error.message)}</p>}
  </form>;
}
function PathField({ id, label, value, onChange, placeholder, actionLabel, invalid, onBrowse }: { id: string; label: string; value: string; onChange: (value: string) => void; placeholder: string; actionLabel: string; invalid: boolean; onBrowse: () => Promise<void> }) { return <section><label htmlFor={id} className="mb-2 block text-sm font-medium text-slate-700">{label}</label><div className="flex flex-col gap-2 sm:flex-row"><Input id={id} className="min-w-0 flex-1" value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} aria-invalid={invalid || undefined} aria-describedby={invalid ? "main-form-error" : undefined} /><Button type="button" variant="outline" className="w-full sm:w-auto" onClick={() => void onBrowse()}><FolderOpen className="size-4" />{actionLabel}</Button></div></section>; }
function CheckOption({ checked, onChange, label, disabled = false }: { checked: boolean; onChange: (value: boolean) => void; label: string; disabled?: boolean }) { return <label className={cn("flex cursor-pointer items-center gap-2.5 text-sm select-none text-slate-800", disabled && "cursor-not-allowed opacity-50")}><input className="sr-only" type="checkbox" checked={checked} disabled={disabled} onChange={(event) => !disabled && onChange(event.target.checked)} /><span className={cn("flex size-4 shrink-0 items-center justify-center rounded-md border transition-all shadow-2xs", checked ? "border-primary bg-primary text-white" : "border-slate-300 bg-white hover:border-pink-300", disabled && "border-slate-200 bg-slate-100")}>{checked && <Check className="size-3.5 stroke-[3] text-white translate-y-px" />}</span><span>{label}</span></label>; }
function SelectOption({ label, value, onChange, options }: { label: string; value: string; onChange: (value: string) => void; options: [string, string][] }) { return <label className="block text-sm font-medium text-slate-700">{label}<select value={value} onChange={(event) => onChange(event.target.value)} className="mt-2 h-10 w-full rounded-lg border border-slate-300 bg-white px-3 text-sm focus:border-primary focus:outline-none">{options.map(([optionValue, optionLabel]) => <option key={optionValue} value={optionValue}>{optionLabel}</option>)}</select></label>; }
