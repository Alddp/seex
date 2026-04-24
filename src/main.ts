import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";

interface AppState {
  history: [string, string][];
  matched: [string, string][];
  keyword: string;
  nlbn_output_path: string;
  nlbn_last_result: string | null;
  nlbn_show_terminal: boolean;
  nlbn_parallel: number;
  nlbn_running: boolean;
  nlbn_path_mode?: string | null;
  npnp_output_path: string;
  npnp_last_result: string | null;
  npnp_running: boolean;
  npnp_mode: NpnpMode;
  npnp_merge: boolean;
  npnp_append: boolean;
  npnp_library_name: string;
  npnp_parallel: number;
  npnp_continue_on_error: boolean;
  npnp_force: boolean;
  monitoring: boolean;
  history_count: number;
  matched_count: number;
  history_save_path: string;
  matched_save_path: string;
}

type ExportTool = "nlbn" | "npnp";
type ExportMessageKind = "info" | "warn" | "success" | "error";

interface ExportFinishedPayload {
  tool: ExportTool;
  success: boolean;
  message: string;
}

interface ExportProgressPayload {
  tool: ExportTool;
  message: string;
  determinate: boolean;
  current: number | null;
  total: number | null;
}

interface ExportNotice {
  kind: ExportMessageKind;
  message: string;
}

interface ExportProgressState {
  determinate: boolean;
  current: number;
  total: number;
  message: string;
}

type Lang = "en" | "zh";
type NpnpMode = "full" | "schlib" | "pcblib";
type Nlbn3dPathMode = "auto" | "project_relative" | "library_relative";

interface ExportCardOptions {
  tool: ExportTool;
  countId: string;
  buttonId: string;
  matchedCount: number;
  running: boolean;
  exportLabelKey: string;
  runningLabelKey: string;
  statusId: string;
  resultId: string;
  result: string | null;
}

const npnpModes: NpnpMode[] = ["full", "schlib", "pcblib"];
const nlbn3dModes: { id: string; value: Nlbn3dPathMode }[] = [
  { id: "btn-nlbn-3d-mode-auto", value: "auto" },
  { id: "btn-nlbn-3d-mode-project", value: "project_relative" },
  { id: "btn-nlbn-3d-mode-library", value: "library_relative" },
];

const enTranslations: Record<string, string> = {
  "nav.monitor": "Monitor",
  "nav.history": "History",
  "nav.export": "Export",
  "nav.language": "Language",
  "nav.about": "About",
  "status.listening": "Listening",
  "monitor.matchMode": "Match Mode",
  "monitor.quickId": "Quick ID",
  "monitor.fullInfo": "Full Info",
  "monitor.monitoring": "Monitoring",
  "monitor.paused": "Paused",
  "monitor.matched": "Matched",
  "monitor.copyIds": "Copy IDs",
  "monitor.show": "Show",
  "monitor.hide": "Hide",
  "monitor.noMatches": "No matches yet",
  "monitor.clipboard": "Clipboard",
  "monitor.waiting": "Waiting for clipboard...",
  "monitor.saveHistory": "Save History",
  "monitor.exportMatched": "Export Matched",
  "monitor.savePaths": "Save paths",
  "monitor.savePathsHint": "Used by Save History and Export Matched.",
  "monitor.historySavePath": "Save History file:",
  "monitor.matchedSavePath": "Export Matched file:",
  "monitor.savePathsExample": "Example: C:\\Users\\xxx\\Documents\\history.txt",
  "monitor.clearAll": "Clear All",
  "monitor.sure": "Sure?",
  "monitor.yes": "Yes",
  "monitor.no": "No",
  "monitor.latest": "Latest:",
  "monitor.copy": "Copy",
  "monitor.delete": "Delete",
  "history.desc": "Clipboard history",
  "history.entries": "entries",
  "history.empty": "No history yet",
  "export.desc": "Component export integrations",
  "export.nlbnExport": "nlbn Export",
  "export.npnpExport": "npnp Export",
  "export.nlbnConfig": "nlbn Configuration",
  "export.npnpConfig": "npnp Configuration",
  "export.itemsReady": "items ready",
  "export.exportNlbn": "Export nlbn",
  "export.exportNpnp": "Export npnp",
  "export.running": "Running...",
  "export.nlbnRunning": "nlbn is running, please wait...",
  "export.npnpRunning": "npnp is running, please wait...",
  "export.exportDir": "Export directory:",
  "export.browse": "Browse",
  "export.apply": "Apply",
  "export.toggleTerminal": "Toggle Terminal",
  "export.terminalOn": "Terminal: ON",
  "export.terminalOff": "Terminal: OFF",
  "export.nlbn3dPathMode": "3D path mode:",
  "export.nlbn3dModeAuto": "Auto",
  "export.nlbn3dModeProject": "KiCad Project",
  "export.nlbn3dModeLibrary": "Library Relative",
  "export.nlbn3dModeHint": "Auto follows export directory detection. Choose an explicit mode to override KiCad 3D path generation.",
  "export.example": "Example: C:\\Users\\xxx\\lib",
  "export.nlbnNotFound": "nlbn is not installed",
  "export.nlbnInstallHint": "Install nlbn and add it to your system PATH to use this feature.",
  "export.npnpMode": "Export mode:",
  "export.npnpOptions": "Batch options:",
  "export.full": "Full",
  "export.schlib": "SchLib",
  "export.pcblib": "PcbLib",
  "export.merge": "Merge",
  "export.mergeAppend": "Merge&Append",
  "export.mergeAppendHint": "Merge&Append extends an existing merged library and skips duplicate IDs (requires Merge).",
  "export.nlbnFor": "nlbn Export for KiCad",
  "export.npnpFor": "npnp Export for Altium Designer",
  "export.libraryName": "Library name:",
  "export.libraryNameHint": "Used as the merged SchLib/PcbLib file name when Merge is enabled.",
  "export.parallel": "Parallel jobs:",
  "export.nlbnParallelHint": "nlbn requires --parallel to be at least 1.",
  "export.npnpParallelHint": "Controls npnp batch concurrency and must be at least 1.",
  "export.continueOnError": "Continue On Error",
  "export.force": "Force",
  "language.desc": "Switch interface language",
  "language.select": "Select Language",
  "about.tagline": "Clipboard Event Tracker",
  "about.desc": "Monitors clipboard in real time, extracts component IDs using keyword or regex, and exports via nlbn or npnp.",
  "status.keyword": "Keyword:",
  "status.none": "none",
};

const zhTranslations: Record<string, string> = {
  ...enTranslations,
  "nav.monitor": "\u76d1\u542c",
  "nav.history": "\u5386\u53f2",
  "nav.export": "\u5bfc\u51fa",
  "nav.language": "\u8bed\u8a00",
  "nav.about": "\u5173\u4e8e",
  "status.listening": "\u76d1\u542c\u4e2d",
  "monitor.matchMode": "\u5339\u914d\u6a21\u5f0f",
  "monitor.quickId": "\u5feb\u901f ID",
  "monitor.fullInfo": "\u5b8c\u6574\u4fe1\u606f",
  "monitor.monitoring": "\u76d1\u542c\u4e2d",
  "monitor.paused": "\u5df2\u6682\u505c",
  "monitor.matched": "\u5339\u914d\u7ed3\u679c",
  "monitor.copyIds": "\u590d\u5236 ID",
  "monitor.show": "\u663e\u793a",
  "monitor.hide": "\u9690\u85cf",
  "monitor.noMatches": "\u6682\u65e0\u5339\u914d\u7ed3\u679c",
  "monitor.clipboard": "\u526a\u8d34\u677f",
  "monitor.waiting": "\u7b49\u5f85\u526a\u8d34\u677f\u5185\u5bb9...",
  "monitor.saveHistory": "\u4fdd\u5b58\u5386\u53f2",
  "monitor.exportMatched": "\u5bfc\u51fa\u5339\u914d",
  "monitor.savePaths": "\u4fdd\u5b58\u8def\u5f84",
  "monitor.savePathsHint": "\u7531\u201c\u4fdd\u5b58\u5386\u53f2\u201d\u548c\u201c\u5bfc\u51fa\u5339\u914d\u201d\u4f7f\u7528\u3002",
  "monitor.historySavePath": "\u4fdd\u5b58\u5386\u53f2\u6587\u4ef6:",
  "monitor.matchedSavePath": "\u5bfc\u51fa\u5339\u914d\u6587\u4ef6:",
  "monitor.savePathsExample": "\u793a\u4f8b: C:\\Users\\xxx\\Documents\\history.txt",
  "monitor.clearAll": "\u6e05\u7a7a\u5168\u90e8",
  "monitor.sure": "\u786e\u5b9a\u5417\uff1f",
  "monitor.yes": "\u662f",
  "monitor.no": "\u5426",
  "monitor.latest": "\u6700\u65b0:",
  "monitor.copy": "\u590d\u5236",
  "monitor.delete": "\u5220\u9664",
  "history.desc": "\u526a\u8d34\u677f\u5386\u53f2",
  "history.entries": "\u6761",
  "history.empty": "\u6682\u65e0\u5386\u53f2\u8bb0\u5f55",
  "export.desc": "\u5143\u4ef6\u5bfc\u51fa\u96c6\u6210",
  "export.nlbnExport": "nlbn \u5bfc\u51fa",
  "export.npnpExport": "npnp \u5bfc\u51fa",
  "export.nlbnConfig": "nlbn \u914d\u7f6e",
  "export.npnpConfig": "npnp \u914d\u7f6e",
  "export.itemsReady": "\u9879\u5f85\u5bfc\u51fa",
  "export.exportNlbn": "\u5bfc\u51fa nlbn",
  "export.exportNpnp": "\u5bfc\u51fa npnp",
  "export.running": "\u8fd0\u884c\u4e2d...",
  "export.nlbnRunning": "nlbn \u6b63\u5728\u8fd0\u884c\uff0c\u8bf7\u7a0d\u5019...",
  "export.npnpRunning": "npnp \u6b63\u5728\u8fd0\u884c\uff0c\u8bf7\u7a0d\u5019...",
  "export.exportDir": "\u5bfc\u51fa\u76ee\u5f55:",
  "export.browse": "\u6d4f\u89c8",
  "export.apply": "\u5e94\u7528",
  "export.toggleTerminal": "\u5207\u6362\u7ec8\u7aef",
  "export.terminalOn": "\u7ec8\u7aef: \u5f00",
  "export.terminalOff": "\u7ec8\u7aef: \u5173",
  "export.nlbn3dPathMode": "3D \u8def\u5f84\u6a21\u5f0f:",
  "export.nlbn3dModeAuto": "\u81ea\u52a8",
  "export.nlbn3dModeProject": "KiCad \u9879\u76ee",
  "export.nlbn3dModeLibrary": "\u5e93\u76f8\u5bf9",
  "export.nlbn3dModeHint": "\u81ea\u52a8\u6a21\u5f0f\u4f1a\u6839\u636e\u5bfc\u51fa\u76ee\u5f55\u63a8\u65ad\u8def\u5f84\u7b56\u7565\uff0c\u4e5f\u53ef\u624b\u52a8\u6307\u5b9a KiCad 3D \u8def\u5f84\u751f\u6210\u65b9\u5f0f\u3002",
  "export.example": "\u793a\u4f8b: C:\\Users\\xxx\\lib",
  "export.nlbnNotFound": "\u672a\u5b89\u88c5 nlbn",
  "export.nlbnInstallHint": "\u8bf7\u5148\u5b89\u88c5 nlbn\uff0c\u5e76\u5c06\u5176\u52a0\u5165\u7cfb\u7edf PATH \u540e\u518d\u4f7f\u7528\u6b64\u529f\u80fd\u3002",
  "export.npnpMode": "\u5bfc\u51fa\u6a21\u5f0f:",
  "export.npnpOptions": "\u6279\u5904\u7406\u9009\u9879:",
  "export.full": "\u5b8c\u6574",
  "export.merge": "\u5408\u5e76",
  "export.mergeAppend": "\u5408\u5e76\u8ffd\u52a0",
  "export.mergeAppendHint": "\u5408\u5e76\u8ffd\u52a0\u4f1a\u5728\u73b0\u6709\u7684\u5408\u5e76\u5e93\u57fa\u7840\u4e0a\u8ffd\u52a0\u5143\u4ef6\u5e76\u8df3\u8fc7\u91cd\u590d ID\uff08\u9700\u8981\u540c\u65f6\u542f\u7528\u5408\u5e76\uff09\u3002",
  "export.nlbnFor": "nlbn KiCad \u5bfc\u51fa",
  "export.npnpFor": "npnp Altium Designer \u5bfc\u51fa",
  "export.libraryName": "\u5e93\u540d\u79f0:",
  "export.libraryNameHint": "\u542f\u7528\u5408\u5e76\u65f6\u4f5c\u4e3a\u5408\u5e76 SchLib/PcbLib \u6587\u4ef6\u540d\u3002",
  "export.parallel": "\u5e76\u884c\u4efb\u52a1\u6570:",
  "export.nlbnParallelHint": "nlbn \u8981\u6c42 --parallel \u81f3\u5c11\u4e3a 1\u3002",
  "export.npnpParallelHint": "\u63a7\u5236 npnp \u6279\u91cf\u5bfc\u51fa\u5e76\u53d1\u6570\uff0c\u4e14\u81f3\u5c11\u4e3a 1\u3002",
  "export.continueOnError": "\u51fa\u9519\u7ee7\u7eed",
  "export.force": "\u5f3a\u5236",
  "language.desc": "\u5207\u6362\u754c\u9762\u8bed\u8a00",
  "language.select": "\u9009\u62e9\u8bed\u8a00",
  "about.tagline": "\u526a\u8d34\u677f\u4e8b\u4ef6\u8ffd\u8e2a\u5668",
  "about.desc": "\u5b9e\u65f6\u76d1\u542c\u526a\u8d34\u677f\uff0c\u6309\u5173\u952e\u5b57\u6216\u6b63\u5219\u63d0\u53d6\u5143\u4ef6 ID\uff0c\u5e76\u901a\u8fc7 nlbn \u6216 npnp \u5bfc\u51fa\u3002",
  "status.keyword": "\u5173\u952e\u5b57:",
  "status.none": "\u65e0",
};

const translations: Record<Lang, Record<string, string>> = {
  en: enTranslations,
  zh: zhTranslations,
};

let currentLang: Lang = "en";
let showMatched = true;
let showHistory = true;
let matchQuick = true;
let matchFull = true;
let lastState: AppState | null = null;

const exportUi: Record<ExportTool, { progress: ExportProgressState | null; notice: ExportNotice | null; resultKind: ExportMessageKind }> = {
  nlbn: { progress: null, notice: null, resultKind: "info" },
  npnp: { progress: null, notice: null, resultKind: "info" },
};

const nlbnUiState: {
  mode: Nlbn3dPathMode;
} = {
  mode: "auto",
};

const PATTERN_QUICK = "regex:(?m)^(C\\d{3,})$";
const PATTERN_FULL = "regex:\u7f16\u53f7[\uff1a:]\\s*(C\\d+)";

function normalizeNlbn3dPathMode(value: unknown): Nlbn3dPathMode | null {
  if (typeof value !== "string") return null;
  const normalized = value.trim().toLowerCase().replace(/[-\s]/g, "_");
  if (normalized === "auto") return "auto";
  if (["project_relative", "project", "kicad_project"].includes(normalized)) return "project_relative";
  if (["library_relative", "library", "relative"].includes(normalized)) return "library_relative";
  return null;
}

function t(key: string): string {
  return translations[currentLang][key] ?? translations.en[key] ?? key;
}

function $(id: string): HTMLElement {
  return document.getElementById(id)!;
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/\"/g, "&quot;");
}

function escapeAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#39;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function buildKeyword(): string {
  const parts: string[] = [];
  if (matchFull) parts.push(PATTERN_FULL);
  if (matchQuick) parts.push(PATTERN_QUICK);
  return parts.join("||");
}

function applyLanguage(lang: Lang) {
  currentLang = lang;
  localStorage.setItem("seex-lang", lang);

  document.documentElement.classList.toggle("lang-zh", lang === "zh");
  document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";

  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n")!;
    el.textContent = t(key);
  });

  document.querySelectorAll("[data-i18n-placeholder]").forEach((el) => {
    const key = el.getAttribute("data-i18n-placeholder")!;
    (el as HTMLInputElement).placeholder = t(key);
  });

  $("btn-lang-en").classList.toggle("active", lang === "en");
  $("btn-lang-zh").classList.toggle("active", lang === "zh");
  $("btn-toggle-matched").textContent = showMatched ? t("monitor.show") : t("monitor.hide");
  rerenderState();
}

function switchPage(pageName: string) {
  document.querySelectorAll(".page").forEach((p) => p.classList.remove("active"));
  document.querySelectorAll(".nav-item").forEach((n) => n.classList.remove("active"));

  const page = document.getElementById(`page-${pageName}`);
  const nav = document.querySelector(`.nav-item[data-page="${pageName}"]`);
  if (page) page.classList.add("active");
  if (nav) nav.classList.add("active");
}

function syncInputValue(id: string, serverValue: string) {
  const input = $(id) as HTMLInputElement;
  const syncedValue = input.dataset.syncedValue;

  if (syncedValue === undefined) {
    input.value = serverValue;
    input.dataset.syncedValue = serverValue;
    return;
  }

  const hasLocalDraft = input.value !== syncedValue;
  if (!hasLocalDraft || input.value === serverValue) {
    input.value = serverValue;
    input.dataset.syncedValue = serverValue;
  }
}

function toolElementId(tool: ExportTool, suffix: string): string {
  return `${tool}-${suffix}`;
}

function messageClass(kind: ExportMessageKind): string {
  switch (kind) {
    case "warn":
      return "msg-warn";
    case "success":
      return "msg-success";
    case "error":
      return "msg-error";
    default:
      return "msg-info";
  }
}

function rerenderState() {
  if (lastState) {
    renderState(lastState);
  }
}

function setExportNotice(tool: ExportTool, message: string | null, kind: ExportMessageKind = "warn") {
  exportUi[tool].notice = message ? { kind, message } : null;
  rerenderState();
}

function startExportProgress(tool: ExportTool, message: string) {
  exportUi[tool].notice = null;
  exportUi[tool].progress = {
    determinate: false,
    current: 0,
    total: 0,
    message,
  };
  exportUi[tool].resultKind = "info";
  rerenderState();
}

function updateExportProgress(payload: ExportProgressPayload) {
  exportUi[payload.tool].notice = null;
  exportUi[payload.tool].progress = {
    determinate: payload.determinate,
    current: payload.current ?? 0,
    total: payload.total ?? 0,
    message: payload.message,
  };
  rerenderState();
}

function finishExportProgress(payload: ExportFinishedPayload) {
  exportUi[payload.tool].progress = null;
  exportUi[payload.tool].notice = null;
  exportUi[payload.tool].resultKind = payload.success ? "success" : "error";
  rerenderState();
}

function renderExportProgress(tool: ExportTool, running: boolean, fallbackMessage: string) {
  const container = $(toolElementId(tool, "progress"));
  const message = $(toolElementId(tool, "progress-message"));
  const meta = $(toolElementId(tool, "progress-meta"));
  const bar = $(toolElementId(tool, "progress-bar")) as HTMLDivElement;
  const progress =
    exportUi[tool].progress ??
    (running
      ? {
          determinate: false,
          current: 0,
          total: 0,
          message: fallbackMessage,
        }
      : null);

  if (!progress) {
    container.classList.add("hidden");
    container.classList.remove("indeterminate");
    message.textContent = "";
    meta.textContent = "";
    bar.style.width = "0%";
    return;
  }

  const determinate = progress.determinate && progress.total > 0;
  const current = determinate ? Math.min(progress.current, progress.total) : 0;
  const width = determinate ? `${Math.max(8, Math.round((current / progress.total) * 100))}%` : "42%";

  container.classList.remove("hidden");
  container.classList.toggle("indeterminate", !determinate);
  message.textContent = progress.message;
  meta.textContent = determinate ? `${current}/${progress.total}` : "";
  bar.style.width = width;
}

function renderExportNotice(tool: ExportTool) {
  const status = $(toolElementId(tool, "status"));
  const notice = exportUi[tool].notice;
  if (!notice) {
    status.textContent = "";
    status.className = "msg msg-warn hidden";
    return;
  }

  status.textContent = notice.message;
  status.className = `msg ${messageClass(notice.kind)}`;
}

function renderExportResult(tool: ExportTool, result: string | null, busy: boolean) {
  const resultBox = $(toolElementId(tool, "result"));
  const promptVisible = tool === "nlbn" && !$("nlbn-not-found").classList.contains("hidden");
  if (!result || busy || exportUi[tool].notice !== null || promptVisible) {
    resultBox.textContent = "";
    resultBox.className = "msg msg-info hidden";
    return;
  }

  resultBox.textContent = result;
  resultBox.className = `msg ${messageClass(exportUi[tool].resultKind)}`;
}

function renderExporterCard(options: ExportCardOptions) {
  $(options.countId).textContent = `${options.matchedCount} ${t("export.itemsReady")}`;

  const busy = options.running || exportUi[options.tool].progress !== null;
  const button = $(options.buttonId) as HTMLButtonElement;
  button.disabled = options.matchedCount === 0 || busy;
  button.textContent = busy ? t("export.running") : t(options.exportLabelKey);

  renderExportProgress(options.tool, busy, t(options.runningLabelKey));
  renderExportNotice(options.tool);
  renderExportResult(options.tool, options.result, busy);
}

function syncOptionalNlbnState(state: AppState) {
  const mode = normalizeNlbn3dPathMode(state.nlbn_path_mode);
  if (mode) {
    nlbnUiState.mode = mode;
  }
}

function renderNlbn3dMode() {
  nlbn3dModes.forEach(({ id, value }) => {
    const button = $(id) as HTMLButtonElement;
    button.classList.toggle("active", nlbnUiState.mode === value);
  });
}

function renderState(state: AppState) {
  syncOptionalNlbnState(state);

  const kwLabel = t("status.keyword");
  const noneLabel = t("status.none");

  $("status-keyword").textContent = `${kwLabel} ${state.keyword || noneLabel}`;
  $("status-counts").textContent = `H: ${state.history_count} | M: ${state.matched_count}`;
  $("monitor-status").textContent = `${kwLabel} ${state.keyword ? "LCSC" : noneLabel} | H: ${state.history_count} | M: ${state.matched_count}`;

  syncInputValue("nlbn-path-input", state.nlbn_output_path);
  syncInputValue("nlbn-parallel-input", String(state.nlbn_parallel));
  syncInputValue("npnp-path-input", state.npnp_output_path);
  syncInputValue("npnp-library-name-input", state.npnp_library_name);
  syncInputValue("npnp-parallel-input", String(state.npnp_parallel));
  syncInputValue("history-save-path-input", state.history_save_path);
  syncInputValue("matched-save-path-input", state.matched_save_path);

  $("nlbn-terminal-status").textContent = state.nlbn_show_terminal ? t("export.terminalOn") : t("export.terminalOff");
  renderNlbn3dMode();

  const monBtn = $("btn-toggle-monitor");
  monBtn.classList.toggle("active", state.monitoring);
  monBtn.textContent = state.monitoring ? t("monitor.monitoring") : t("monitor.paused");

  renderExporterCard({
    tool: "nlbn",
    countId: "nlbn-export-count",
    buttonId: "btn-nlbn-export",
    matchedCount: state.matched_count,
    running: state.nlbn_running,
    exportLabelKey: "export.exportNlbn",
    runningLabelKey: "export.nlbnRunning",
    statusId: "nlbn-status",
    resultId: "nlbn-result",
    result: state.nlbn_last_result,
  });

  renderExporterCard({
    tool: "npnp",
    countId: "npnp-export-count",
    buttonId: "btn-npnp-export",
    matchedCount: state.matched_count,
    running: state.npnp_running,
    exportLabelKey: "export.exportNpnp",
    runningLabelKey: "export.npnpRunning",
    statusId: "npnp-status",
    resultId: "npnp-result",
    result: state.npnp_last_result,
  });

  npnpModes.forEach((mode) => {
    $("btn-npnp-mode-" + mode).classList.toggle("active", state.npnp_mode === mode);
  });

  $("btn-toggle-npnp-merge").classList.toggle("active", state.npnp_merge);
  $("btn-toggle-npnp-append").classList.toggle("active", state.npnp_append);
  $("btn-toggle-npnp-continue-on-error").classList.toggle("active", state.npnp_continue_on_error);
  $("btn-toggle-npnp-force").classList.toggle("active", state.npnp_force);

  const libraryInput = $("npnp-library-name-input") as HTMLInputElement;
  const libraryApply = $("btn-apply-npnp-library-name") as HTMLButtonElement;
  libraryInput.disabled = !state.npnp_merge;
  libraryApply.disabled = !state.npnp_merge;

  const parallelInput = $("npnp-parallel-input") as HTMLInputElement;
  const parallelApply = $("btn-apply-npnp-parallel") as HTMLButtonElement;
  parallelInput.disabled = false;
  parallelApply.disabled = false;

  const forceToggle = $("btn-toggle-npnp-force") as HTMLButtonElement;
  forceToggle.disabled = false;

  $("matched-count").textContent = String(state.matched_count);
  if (showMatched && state.matched.length > 0) {
    $("matched-list").classList.remove("hidden");
    $("matched-empty").classList.add("hidden");
    renderMatchedList(state.matched);
  } else if (state.matched.length === 0) {
    $("matched-list").classList.add("hidden");
    $("matched-empty").classList.remove("hidden");
  } else {
    $("matched-list").classList.add("hidden");
    $("matched-empty").classList.add("hidden");
  }

  if (state.history.length > 0) {
    $("latest-preview").classList.remove("hidden");
    $("history-waiting").classList.add("hidden");
    const [time, content] = state.history[0];
    $("latest-time").textContent = `${t("monitor.latest")} ${time}`;
    ($("latest-content") as HTMLTextAreaElement).value = content;
  } else {
    $("latest-preview").classList.add("hidden");
    $("history-waiting").classList.remove("hidden");
  }

  $("history-count-badge").textContent = String(state.history_count);
  if (showHistory && state.history.length > 0) {
    $("history-list").classList.remove("hidden");
    $("history-empty").classList.add("hidden");
    renderHistoryList(state.history);
  } else if (state.history.length === 0) {
    $("history-list").classList.add("hidden");
    $("history-empty").classList.remove("hidden");
  } else {
    $("history-list").classList.add("hidden");
    $("history-empty").classList.add("hidden");
  }
}

function renderMatchedList(items: [string, string][]) {
  const copyLabel = t("monitor.copy");
  const c = $("matched-list");
  c.innerHTML = "";
  items.forEach(([time, value], idx) => {
    const row = document.createElement("div");
    row.className = "item-row";
    row.innerHTML = `
      <span class="item-time">${escapeHtml(time)}</span>
      <span class="item-value">${escapeHtml(value)}</span>
      <span class="item-actions">
        <button data-copy="${escapeAttr(value)}" title="${copyLabel}">${copyLabel}</button>
        <button data-delete-matched="${idx}" title="${t("monitor.delete")}">&times;</button>
      </span>`;
    c.appendChild(row);
  });
}

function renderHistoryList(items: [string, string][]) {
  const copyLabel = t("monitor.copy");
  const c = $("history-list");
  c.innerHTML = "";
  items.forEach(([time, content], idx) => {
    const preview = content.split("\n")[0].substring(0, 80);
    const div = document.createElement("div");
    div.className = "history-item";
    div.innerHTML = `
      <div class="item-row">
        <span class="item-time">${escapeHtml(time)}</span>
        <span class="item-value">${escapeHtml(preview)}</span>
        <span class="item-actions">
          <button data-copy="${escapeAttr(content)}" title="${copyLabel}">${copyLabel}</button>
          <button data-delete-history="${idx}" title="${t("monitor.delete")}">&times;</button>
        </span>
      </div>`;
    c.appendChild(div);
  });
}

async function refreshState() {
  const state: AppState = await invoke("get_state");
  lastState = state;
  renderState(state);
}

async function selectDirectory(title: string): Promise<string | null> {
  const selected = await open({ directory: true, title });
  return typeof selected === "string" ? selected : null;
}

async function selectSaveFile(title: string, defaultPath: string | undefined): Promise<string | null> {
  const selected = await save({
    title,
    defaultPath: defaultPath && defaultPath.trim().length > 0 ? defaultPath : undefined,
    filters: [
      { name: "Text", extensions: ["txt"] },
      { name: "All files", extensions: ["*"] },
    ],
  });
  return typeof selected === "string" ? selected : null;
}

function parsePositiveIntOrFallback(value: string, fallback: number): number {
  const parsed = Number.parseInt(value.trim(), 10);
  return Number.isFinite(parsed) && parsed >= 1 ? parsed : fallback;
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

let monitorSaveResultTimer: number | null = null;

function showMonitorSaveResult(message: string, kind?: ExportMessageKind) {
  const el = $("monitor-save-result");
  const resolvedKind: ExportMessageKind = kind ?? classifySaveResult(message);
  el.textContent = message;
  el.className = `msg ${messageClass(resolvedKind)}`;

  if (monitorSaveResultTimer !== null) {
    window.clearTimeout(monitorSaveResultTimer);
  }
  monitorSaveResultTimer = window.setTimeout(() => {
    el.textContent = "";
    el.className = "msg msg-info hidden";
    monitorSaveResultTimer = null;
  }, 6000);
}

function classifySaveResult(message: string): ExportMessageKind {
  const lower = message.toLowerCase();
  if (lower.startsWith("saved") || lower.startsWith("exported")) return "success";
  if (lower.includes("failed")) return "error";
  return "warn";
}

function showExportStartResult(tool: ExportTool, result: string): boolean {
  if (result === "Export started") {
    setExportNotice(tool, null);
    return true;
  }

  exportUi[tool].progress = null;
  exportUi[tool].notice = { kind: "warn", message: result };
  rerenderState();
  return false;
}

function showExportError(tool: ExportTool, error: string) {
  exportUi[tool].progress = null;
  exportUi[tool].notice = { kind: "error", message: error };
  rerenderState();
}

let pendingExportConfigWrite: Promise<void> = Promise.resolve();

function queueExportConfigWrite(operation: () => Promise<void>): Promise<void> {
  const run = pendingExportConfigWrite.then(operation, operation);
  pendingExportConfigWrite = run.catch(() => {});
  return run;
}

async function syncNlbnExportInputs() {
  const path = ($("nlbn-path-input") as HTMLInputElement).value;
  const parallelValue = ($("nlbn-parallel-input") as HTMLInputElement).value;
  const parallel = parsePositiveIntOrFallback(parallelValue, 4);

  await invoke("set_nlbn_path", { path });
  await invoke("set_nlbn_parallel", { parallel });
}

async function syncNpnpExportInputs() {
  const path = ($("npnp-path-input") as HTMLInputElement).value;
  const libraryName = ($("npnp-library-name-input") as HTMLInputElement).value;
  const parallelValue = ($("npnp-parallel-input") as HTMLInputElement).value;
  const parallel = parsePositiveIntOrFallback(parallelValue, 4);

  await invoke("set_npnp_path", { path });
  await invoke("set_npnp_library_name", { libraryName });
  await invoke("set_npnp_parallel", { parallel });
}

async function setNlbn3dMode(mode: Nlbn3dPathMode) {
  await invoke("set_nlbn_path_mode", { pathMode: mode });
  nlbnUiState.mode = mode;
  setExportNotice("nlbn", null);
  await refreshState();
}

window.addEventListener("DOMContentLoaded", async () => {
  const savedLang = localStorage.getItem("seex-lang") as Lang | null;
  if (savedLang === "zh" || savedLang === "en") {
    applyLanguage(savedLang);
  }

  await refreshState();
  await listen("clipboard-changed", () => {
    void refreshState();
  });
  await listen<ExportProgressPayload>("export-progress", (event) => {
    updateExportProgress(event.payload);
  });
  await listen<ExportFinishedPayload>("export-finished", async (event) => {
    finishExportProgress(event.payload);
    await refreshState();
  });

  document.querySelectorAll(".nav-item").forEach((item) => {
    item.addEventListener("click", () => {
      const page = item.getAttribute("data-page");
      if (page) switchPage(page);
    });
  });

  $("btn-collapse").addEventListener("click", () => {
    $("sidebar").classList.toggle("collapsed");
  });

  $("btn-lang-en").addEventListener("click", () => {
    applyLanguage("en");
    void refreshState();
  });

  $("btn-lang-zh").addEventListener("click", () => {
    applyLanguage("zh");
    void refreshState();
  });

  $("btn-match-quick").addEventListener("click", async () => {
    matchQuick = !matchQuick;
    $("btn-match-quick").classList.toggle("active", matchQuick);
    await invoke("set_keyword", { keyword: buildKeyword() });
    await refreshState();
  });

  $("btn-match-full").addEventListener("click", async () => {
    matchFull = !matchFull;
    $("btn-match-full").classList.toggle("active", matchFull);
    await invoke("set_keyword", { keyword: buildKeyword() });
    await refreshState();
  });

  $("btn-toggle-monitor").addEventListener("click", async () => {
    await invoke("toggle_monitoring");
    await refreshState();
  });

  $("btn-toggle-matched").addEventListener("click", () => {
    showMatched = !showMatched;
    $("btn-toggle-matched").classList.toggle("active", showMatched);
    $("btn-toggle-matched").textContent = showMatched ? t("monitor.show") : t("monitor.hide");
    void refreshState();
  });

  $("btn-copy-ids").addEventListener("click", async () => {
    const ids: string[] = await invoke("get_unique_ids");
    if (ids.length > 0) {
      await invoke("copy_to_clipboard", { text: ids.join("\n") });
    }
  });

  $("btn-nlbn-export").addEventListener("click", async () => {
    startExportProgress("nlbn", t("export.nlbnRunning"));
    $("nlbn-not-found").classList.add("hidden");

    try {
      await queueExportConfigWrite(async () => {
        await syncNlbnExportInputs();
        await refreshState();
      });
      await invoke("check_nlbn");
      const result = await invoke<string>("nlbn_export");
      showExportStartResult("nlbn", result);
      await refreshState();
    } catch (error) {
      const details = errorMessage(error);
      if (details.includes("nlbn not found")) {
        exportUi.nlbn.progress = null;
        exportUi.nlbn.notice = null;
        rerenderState();
        $("nlbn-not-found").classList.remove("hidden");
        return;
      }

      showExportError("nlbn", details);
      await refreshState();
    }
  });

  $("btn-browse-nlbn-folder").addEventListener("click", async () => {
    const selected = await selectDirectory("Select nlbn export directory");
    if (selected) {
      ($("nlbn-path-input") as HTMLInputElement).value = selected;
      await queueExportConfigWrite(async () => {
        await invoke("set_nlbn_path", { path: selected });
        await refreshState();
      });
    }
  });

  $("btn-apply-nlbn-path").addEventListener("click", async () => {
    const path = ($("nlbn-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_nlbn_path", { path });
      await refreshState();
    });
  });

  $("btn-toggle-nlbn-terminal").addEventListener("click", async () => {
    await queueExportConfigWrite(async () => {
      await invoke("toggle_nlbn_terminal");
      await refreshState();
    });
  });

  nlbn3dModes.forEach(({ id, value }) => {
    $(id).addEventListener("click", async () => {
      await queueExportConfigWrite(async () => {
        await setNlbn3dMode(value);
      });
    });
  });

  $("btn-apply-nlbn-parallel").addEventListener("click", async () => {
    const value = ($("nlbn-parallel-input") as HTMLInputElement).value;
    const parallel = parsePositiveIntOrFallback(value, 4);
    await queueExportConfigWrite(async () => {
      await invoke("set_nlbn_parallel", { parallel });
      await refreshState();
    });
  });

  $("btn-npnp-export").addEventListener("click", async () => {
    startExportProgress("npnp", t("export.npnpRunning"));

    try {
      await queueExportConfigWrite(async () => {
        await syncNpnpExportInputs();
        await refreshState();
      });
      const result = await invoke<string>("npnp_export");
      showExportStartResult("npnp", result);
      await refreshState();
    } catch (error) {
      showExportError("npnp", errorMessage(error));
      await refreshState();
    }
  });

  $("btn-browse-npnp-folder").addEventListener("click", async () => {
    const selected = await selectDirectory("Select npnp export directory");
    if (selected) {
      ($("npnp-path-input") as HTMLInputElement).value = selected;
      await queueExportConfigWrite(async () => {
        await invoke("set_npnp_path", { path: selected });
        await refreshState();
      });
    }
  });

  $("btn-apply-npnp-path").addEventListener("click", async () => {
    const path = ($("npnp-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_npnp_path", { path });
      await refreshState();
    });
  });

  npnpModes.forEach((mode) => {
    $("btn-npnp-mode-" + mode).addEventListener("click", async () => {
      await queueExportConfigWrite(async () => {
        await invoke("set_npnp_mode", { mode });
        await refreshState();
      });
    });
  });

  $("btn-toggle-npnp-merge").addEventListener("click", async () => {
    const active = $("btn-toggle-npnp-merge").classList.contains("active");
    await queueExportConfigWrite(async () => {
      await invoke("set_npnp_merge", { merge: !active });
      await refreshState();
    });
  });

  $("btn-toggle-npnp-append").addEventListener("click", async () => {
    const active = $("btn-toggle-npnp-append").classList.contains("active");
    await queueExportConfigWrite(async () => {
      await invoke("set_npnp_append", { append: !active });
      await refreshState();
    });
  });

  $("btn-toggle-npnp-continue-on-error").addEventListener("click", async () => {
    const active = $("btn-toggle-npnp-continue-on-error").classList.contains("active");
    await queueExportConfigWrite(async () => {
      await invoke("set_npnp_continue_on_error", { continueOnError: !active });
      await refreshState();
    });
  });

  $("btn-toggle-npnp-force").addEventListener("click", async () => {
    const active = $("btn-toggle-npnp-force").classList.contains("active");
    await queueExportConfigWrite(async () => {
      await invoke("set_npnp_force", { force: !active });
      await refreshState();
    });
  });

  $("btn-apply-npnp-library-name").addEventListener("click", async () => {
    const libraryName = ($("npnp-library-name-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_npnp_library_name", { libraryName });
      await refreshState();
    });
  });

  $("btn-apply-npnp-parallel").addEventListener("click", async () => {
    const value = ($("npnp-parallel-input") as HTMLInputElement).value;
    const parallel = parsePositiveIntOrFallback(value, 4);
    await queueExportConfigWrite(async () => {
      await invoke("set_npnp_parallel", { parallel });
      await refreshState();
    });
  });

  $("btn-save-history").addEventListener("click", async () => {
    try {
      const result = await invoke<string>("save_history");
      showMonitorSaveResult(result);
    } catch (error) {
      showMonitorSaveResult(errorMessage(error), "error");
    }
  });

  $("btn-apply-history-save-path").addEventListener("click", async () => {
    const path = ($("history-save-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_history_save_path", { path });
      await refreshState();
    });
  });

  $("btn-browse-history-save-path").addEventListener("click", async () => {
    const current = ($("history-save-path-input") as HTMLInputElement).value;
    const selected = await selectSaveFile("Choose Save History file", current);
    if (selected) {
      ($("history-save-path-input") as HTMLInputElement).value = selected;
      await queueExportConfigWrite(async () => {
        await invoke("set_history_save_path", { path: selected });
        await refreshState();
      });
    }
  });

  $("btn-apply-matched-save-path").addEventListener("click", async () => {
    const path = ($("matched-save-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_matched_save_path", { path });
      await refreshState();
    });
  });

  $("btn-browse-matched-save-path").addEventListener("click", async () => {
    const current = ($("matched-save-path-input") as HTMLInputElement).value;
    const selected = await selectSaveFile("Choose Export Matched file", current);
    if (selected) {
      ($("matched-save-path-input") as HTMLInputElement).value = selected;
      await queueExportConfigWrite(async () => {
        await invoke("set_matched_save_path", { path: selected });
        await refreshState();
      });
    }
  });

  $("btn-save-matched").addEventListener("click", async () => {
    try {
      const result = await invoke<string>("save_matched");
      showMonitorSaveResult(result);
    } catch (error) {
      showMonitorSaveResult(errorMessage(error), "error");
    }
  });

  $("btn-clear-all").addEventListener("click", () => {
    $("btn-clear-all").classList.add("hidden");
    $("clear-confirm").classList.remove("hidden");
  });

  $("btn-clear-confirm").addEventListener("click", async () => {
    $("btn-clear-all").classList.remove("hidden");
    $("clear-confirm").classList.add("hidden");
    await invoke("clear_all");
    await refreshState();
  });

  $("btn-clear-cancel").addEventListener("click", () => {
    $("btn-clear-all").classList.remove("hidden");
    $("clear-confirm").classList.add("hidden");
  });

  showHistory = true;

  document.addEventListener("click", async (e) => {
    const target = e.target as HTMLElement;

    const urlEl = target.closest("[data-url]") as HTMLElement | null;
    if (urlEl) {
      const url = urlEl.getAttribute("data-url");
      if (url) {
        await openUrl(url);
        return;
      }
    }

    const copyVal = target.getAttribute("data-copy");
    if (copyVal !== null) {
      await invoke("copy_to_clipboard", { text: copyVal });
      return;
    }

    const dm = target.getAttribute("data-delete-matched");
    if (dm !== null) {
      await invoke("delete_matched", { index: parseInt(dm, 10) });
      await refreshState();
      return;
    }

    const dh = target.getAttribute("data-delete-history");
    if (dh !== null) {
      await invoke("delete_history", { index: parseInt(dh, 10) });
      await refreshState();
    }
  });
});
