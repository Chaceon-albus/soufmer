import * as Collapsible from "@radix-ui/react-collapsible";
import { ChevronDown, FileAudio, Folder, FolderOpen, SlidersHorizontal, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { startBatchRequestSchema } from "@/lib/schemas";
import { cn } from "@/lib/utils";
import type { AppSettings, InputMode, StartBatchRequest } from "@/types/domain";

type Props = { settings: AppSettings; onSubmit: (request: StartBatchRequest) => void; onSettingsChange: (settings: AppSettings) => void; onChooseInput: (mode: InputMode) => Promise<string | null>; onChooseOutput: () => Promise<string | null>; submitDisabled?: boolean };
export function MainForm({ settings, onSubmit, onSettingsChange, onChooseInput, onChooseOutput, submitDisabled = false }: Props) {
  const { t } = useTranslation();
  const [inputMode, setInputMode] = useState<InputMode>(settings.lastInputMode);
  const [inputPath, setInputPath] = useState("");
  const [outputDirectory, setOutputDirectory] = useState(settings.lastOutputDirectory ?? "");
  const [processingMode, setProcessingMode] = useState(settings.processingMode);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [recursive, setRecursive] = useState(settings.recursive);
  const [preserveDirectoryStructure, setPreserveDirectoryStructure] = useState(settings.preserveDirectoryStructure);
  const [generateBothModes, setGenerateBothModes] = useState(settings.generateBothModes);
  const [conflictPolicy, setConflictPolicy] = useState(settings.conflictPolicy);
  const [outputFormat, setOutputFormat] = useState(settings.outputFormat);
  const [error, setError] = useState<string>();
  useEffect(() => { onSettingsChange({ ...settings, lastInputMode: inputMode, lastOutputDirectory: outputDirectory || undefined, processingMode, recursive, preserveDirectoryStructure, generateBothModes, conflictPolicy, outputFormat }); }, [conflictPolicy, generateBothModes, inputMode, onSettingsChange, outputDirectory, outputFormat, preserveDirectoryStructure, processingMode, recursive, settings]);
  const chooseLabel = inputMode === "file" ? t("main.input.chooseFile") : t("main.input.chooseFolder");
  const submit = () => {
    if (submitDisabled) return;
    const parsed = startBatchRequestSchema.safeParse({ inputMode, inputPath, outputDirectory, processingMode, generateBothModes, recursive, preserveDirectoryStructure, conflictPolicy, outputFormat });
    if (!parsed.success) { setError(parsed.error.issues[0]?.message ?? "validation.inputRequired"); return; }
    setError(undefined); onSubmit(parsed.data);
  };
  return <form className="space-y-6" onSubmit={(event) => { event.preventDefault(); submit(); }}>
    <section><p className="mb-2 text-sm font-medium text-slate-700">{t("main.input.type")}</p><div className="grid grid-cols-2 rounded-lg bg-slate-100 p-1" role="radiogroup" aria-label={t("main.input.type")}>{(["file", "folder"] as const).map((mode) => <button key={mode} type="button" role="radio" aria-checked={inputMode === mode} className={cn("flex min-h-11 items-center justify-center gap-2 rounded-md text-sm font-medium focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600", inputMode === mode ? "bg-white text-slate-900 shadow-sm" : "text-slate-600 hover:text-slate-900")} onClick={() => setInputMode(mode)}>{mode === "file" ? <FileAudio className="size-4" /> : <Folder className="size-4" />}{t(`main.input.${mode}`)}</button>)}</div></section>
    <PathField id="input-path" label={t("main.input.path")} value={inputPath} onChange={setInputPath} placeholder={t("main.input.placeholder")} actionLabel={chooseLabel} onBrowse={async () => { const path = await onChooseInput(inputMode); if (path) setInputPath(path); }} />
    <PathField id="output-directory" label={t("main.output.label")} value={outputDirectory} onChange={setOutputDirectory} placeholder={t("main.output.placeholder")} actionLabel={t("main.output.choose")} onBrowse={async () => { const path = await onChooseOutput(); if (path) setOutputDirectory(path); }} />
    <section><p className="mb-3 text-sm font-medium text-slate-700">{t("main.mode.label")}</p><div className="grid gap-3 sm:grid-cols-2">{(["compatibility44100", "sourceSampleRate"] as const).map((mode) => <label key={mode} className={cn("cursor-pointer rounded-xl border p-4 transition-colors focus-within:border-sky-600 focus-within:ring-2 focus-within:ring-sky-100", processingMode === mode ? "border-sky-600 bg-sky-50" : "border-slate-200 hover:border-slate-300")}><input className="sr-only" type="radio" checked={processingMode === mode} onChange={() => setProcessingMode(mode)} /><span className="flex items-start gap-3"><span className={cn("mt-0.5 size-4 rounded-full border-4", processingMode === mode ? "border-sky-700" : "border-slate-300")} /><span><span className="block font-medium">{t(`main.mode.${mode}.title`)}</span><span className="mt-1 block text-sm leading-5 text-slate-600">{t(`main.mode.${mode}.description`)}</span></span></span></label>)}</div></section>
    <Collapsible.Root open={advancedOpen} onOpenChange={setAdvancedOpen}><Collapsible.Trigger asChild><Button type="button" variant="ghost" className="w-full justify-between border border-slate-200"><span className="flex items-center gap-2"><SlidersHorizontal className="size-4" />{t("advanced.title")}</span><ChevronDown className={cn("size-4 transition-transform", advancedOpen && "rotate-180")} /></Button></Collapsible.Trigger><Collapsible.Content className="mt-3 rounded-xl border border-slate-200 p-4"><div className="space-y-4">{inputMode === "folder" && <><CheckOption checked={recursive} onChange={setRecursive} label={t("advanced.recursive")} /><CheckOption checked={preserveDirectoryStructure} onChange={setPreserveDirectoryStructure} label={t("advanced.preserveStructure")} /></>}<CheckOption checked={generateBothModes} onChange={setGenerateBothModes} label={t("advanced.generateBoth")} /><SelectOption label={t("advanced.outputFormat")} value={outputFormat} onChange={(value) => setOutputFormat(value as typeof outputFormat)} options={[["flac", t("advanced.formatFlac")], ["wavFloat32", t("advanced.formatWav")]]} /><SelectOption label={t("advanced.conflictPolicy")} value={conflictPolicy} onChange={(value) => setConflictPolicy(value as typeof conflictPolicy)} options={[["skip", t("advanced.conflictSkip")], ["overwrite", t("advanced.conflictOverwrite")], ["autoNumber", t("advanced.conflictAutoNumber")]]} /></div></Collapsible.Content></Collapsible.Root>
    {error && <p className="text-sm text-red-700" role="alert">{t(error)}</p>}
    <Button type="submit" size="lg" className="w-full" disabled={submitDisabled}><Sparkles className="size-4" />{t("main.start")}</Button>
  </form>;
}
function PathField({ id, label, value, onChange, placeholder, actionLabel, onBrowse }: { id: string; label: string; value: string; onChange: (value: string) => void; placeholder: string; actionLabel: string; onBrowse: () => Promise<void> }) { return <section><label htmlFor={id} className="mb-2 block text-sm font-medium text-slate-700">{label}</label><div className="flex flex-col gap-2 sm:flex-row"><Input id={id} className="min-w-0 flex-1" value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} /><Button type="button" variant="outline" className="w-full sm:w-auto" onClick={() => void onBrowse()}><FolderOpen className="size-4" />{actionLabel}</Button></div></section>; }
function CheckOption({ checked, onChange, label }: { checked: boolean; onChange: (value: boolean) => void; label: string }) { return <label className="flex cursor-pointer items-center gap-3 text-sm"><input className="size-4 accent-sky-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600" type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />{label}</label>; }
function SelectOption({ label, value, onChange, options }: { label: string; value: string; onChange: (value: string) => void; options: [string, string][] }) { return <label className="block text-sm font-medium text-slate-700">{label}<select value={value} onChange={(event) => onChange(event.target.value)} className="mt-2 h-10 w-full rounded-lg border border-slate-300 bg-white px-3 text-sm focus:outline-2 focus:outline-sky-600">{options.map(([optionValue, optionLabel]) => <option key={optionValue} value={optionValue}>{optionLabel}</option>)}</select></label>; }
