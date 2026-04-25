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
  nlbn_export_symbol?: boolean;
  nlbn_export_footprint?: boolean;
  nlbn_export_model_3d?: boolean;
  nlbn_overwrite_symbol?: boolean;
  nlbn_overwrite_footprint?: boolean;
  nlbn_overwrite_model_3d?: boolean;
  nlbn_symbol_fill_color?: string | null;
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
  always_on_top: boolean;
  history_count: number;
  matched_count: number;
  history_save_path: string;
  matched_save_path: string;
  imported_parts_save_path: string;
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

interface ImportedSymbol {
  lcsc_part: string;
  symbol_name: string;
  source_file: string;
}

interface ImportedSymbolsResponse {
  scanned_path: string;
  items: ImportedSymbol[];
}

type NpnpMode = "full" | "schlib" | "pcblib";
type Nlbn3dPathMode = "auto" | "project_relative" | "library_relative";
type NlbnAssetKey = "symbol" | "footprint" | "model_3d";
type NlbnExportField = "nlbn_export_symbol" | "nlbn_export_footprint" | "nlbn_export_model_3d";
type NlbnOverwriteField = "nlbn_overwrite_symbol" | "nlbn_overwrite_footprint" | "nlbn_overwrite_model_3d";
type PageName = "monitor" | "history" | "export" | "imported" | "about";

interface NlbnAssetToggle {
  key: NlbnAssetKey;
  labelKey: string;
  exportField: NlbnExportField;
  overwriteField: NlbnOverwriteField;
  exportButtonId: string;
  overwriteButtonId: string;
  exportCommand: string;
  overwriteCommand: string;
}

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
  buttonDisabled?: boolean;
  derivedNotice?: ExportNotice | null;
}

const npnpModes: NpnpMode[] = ["full", "schlib", "pcblib"];
const nlbn3dModes: { id: string; value: Nlbn3dPathMode }[] = [
  { id: "btn-nlbn-3d-mode-auto", value: "auto" },
  { id: "btn-nlbn-3d-mode-project", value: "project_relative" },
  { id: "btn-nlbn-3d-mode-library", value: "library_relative" },
];

const nlbnAssetToggles: NlbnAssetToggle[] = [
  {
    key: "symbol",
    labelKey: "export.nlbnAssetSymbol",
    exportField: "nlbn_export_symbol",
    overwriteField: "nlbn_overwrite_symbol",
    exportButtonId: "btn-toggle-nlbn-export-symbol",
    overwriteButtonId: "btn-toggle-nlbn-overwrite-symbol",
    exportCommand: "set_nlbn_export_symbol",
    overwriteCommand: "set_nlbn_overwrite_symbol",
  },
  {
    key: "footprint",
    labelKey: "export.nlbnAssetFootprint",
    exportField: "nlbn_export_footprint",
    overwriteField: "nlbn_overwrite_footprint",
    exportButtonId: "btn-toggle-nlbn-export-footprint",
    overwriteButtonId: "btn-toggle-nlbn-overwrite-footprint",
    exportCommand: "set_nlbn_export_footprint",
    overwriteCommand: "set_nlbn_overwrite_footprint",
  },
  {
    key: "model_3d",
    labelKey: "export.nlbnAssetModel3d",
    exportField: "nlbn_export_model_3d",
    overwriteField: "nlbn_overwrite_model_3d",
    exportButtonId: "btn-toggle-nlbn-export-model-3d",
    overwriteButtonId: "btn-toggle-nlbn-overwrite-model-3d",
    exportCommand: "set_nlbn_export_model_3d",
    overwriteCommand: "set_nlbn_overwrite_model_3d",
  },
];

const enTranslations: Record<string, string> = {
  "nav.monitor": "Monitor",
  "nav.history": "History",
  "nav.export": "Export",
  "nav.imported": "Imported",
  "nav.about": "About",
  "status.listening": "Listening",
  "status.alwaysOnTopOn": "Always on Top: ON",
  "status.alwaysOnTopOff": "Always on Top: OFF",
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
  "imported.desc": "Browse imported nlbn symbols from the active KiCad library",
  "imported.title": "Imported Symbols",
  "imported.refresh": "Refresh",
  "imported.importParts": "Import Parts",
  "imported.exportParts": "Export Parts",
  "imported.exportFile": "Parts file:",
  "imported.exportHint": "Used by Export Parts and Import Parts.",
  "imported.exportDialog": "Choose LCSC Parts file",
  "imported.scannedPath": "Scanned directory:",
  "imported.lcscPart": "LCSC Part",
  "imported.symbolName": "Symbol Name",
  "imported.actions": "Actions",
  "imported.loading": "Loading imported symbols...",
  "imported.empty": "No imported symbols found in the current nlbn output library.",
  "imported.noFilterResults": "No imported symbols match the current filter.",
  "imported.copyPart": "Copy LCSC Part",
  "imported.copied": "LCSC Part copied to clipboard.",
  "imported.edit": "Edit",
  "imported.deleteSymbol": "Delete",
  "imported.editorTitle": "Edit Imported Symbol",
  "imported.editorHint": "Update the symbol name or LCSC Part directly in the KiCad symbol library.",
  "imported.editorSourceFile": "Source file:",
  "imported.editorSymbolName": "Symbol Name:",
  "imported.editorLcscPart": "LCSC Part:",
  "imported.save": "Save",
  "imported.cancel": "Cancel",
  "imported.deleteConfirm": "Delete {symbol} from {file}? This also removes the matching footprint, 3D models, and checkpoint entry when present.",
  "imported.search": "Search:",
  "imported.searchPlaceholder": "Filter by LCSC Part or symbol name",
  "imported.total": "Total",
  "imported.filtered": "Filtered",
  "imported.selected": "Selected",
  "imported.selectFiltered": "Select Filtered",
  "imported.clearSelection": "Clear Selection",
  "imported.copyParts": "Copy Parts",
  "imported.queueParts": "Queue Parts",
  "imported.queuePart": "Queue",
  "imported.selectionHint": "Copy, queue, and export use selected parts first; if nothing is selected, the current filtered list is used.",
  "imported.noActionableParts": "No LCSC parts available for this action.",
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
  "export.nlbnExportContent": "Export Content:",
  "export.nlbnOverwriteExisting": "Overwrite Existing:",
  "export.nlbnAssetSymbol": "Symbol",
  "export.nlbnAssetFootprint": "Footprint",
  "export.nlbnAssetModel3d": "3D Model",
  "export.nlbnSelectAtLeastOne": "Enable at least one export content option to run nlbn export.",
  "export.overwriteOn": "Overwrite: ON",
  "export.overwriteOff": "Overwrite: OFF",
  "export.nlbnOverwriteHint": "Overwrite only applies to enabled export content. Turning an export item off will also turn its overwrite option off.",
  "export.nlbnFillColor": "Symbol fill color:",
  "export.nlbnFillColorHint": "Optional. Leave blank to keep nlbn/KiCad defaults. Supports #RRGGBB or #RRGGBBAA.",
  "export.nlbnFillColorPlaceholder": "Example: #005C8FCC",
  "export.nlbnFillColorClear": "Clear",
  "export.nlbnFillColorAuto": "No override",
  "export.nlbnFillColorInvalid": "Use #RRGGBB or #RRGGBBAA.",
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
  "nav.imported": "\u5df2\u5bfc\u5165",
  "nav.about": "\u5173\u4e8e",
  "status.listening": "\u76d1\u542c\u4e2d",
  "status.alwaysOnTopOn": "\u7a97\u53e3\u7f6e\u9876: \u5f00",
  "status.alwaysOnTopOff": "\u7a97\u53e3\u7f6e\u9876: \u5173",
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
  "imported.desc": "\u67e5\u770b\u5f53\u524d nlbn KiCad \u7b26\u53f7\u5e93\u4e2d\u5df2\u5bfc\u5165\u7684\u7b26\u53f7",
  "imported.title": "\u5df2\u5bfc\u5165\u7b26\u53f7",
  "imported.refresh": "\u5237\u65b0",
  "imported.importParts": "\u5bfc\u5165 Part",
  "imported.exportParts": "\u5bfc\u51fa Part",
  "imported.exportFile": "Part \u6587\u4ef6:",
  "imported.exportHint": "\u7528\u4e8e\u201c\u5bfc\u51fa Part\u201d\u548c\u201c\u5bfc\u5165 Part\u201d\u6309\u94ae\u3002",
  "imported.exportDialog": "\u9009\u62e9 LCSC Part \u6587\u4ef6",
  "imported.scannedPath": "\u626b\u63cf\u76ee\u5f55:",
  "imported.lcscPart": "LCSC Part",
  "imported.symbolName": "\u7b26\u53f7\u540d",
  "imported.actions": "\u64cd\u4f5c",
  "imported.loading": "\u6b63\u5728\u52a0\u8f7d\u5df2\u5bfc\u5165\u7b26\u53f7...",
  "imported.empty": "\u5f53\u524d nlbn \u8f93\u51fa\u7b26\u53f7\u5e93\u4e2d\u8fd8\u6ca1\u6709\u627e\u5230\u5df2\u5bfc\u5165\u7b26\u53f7\u3002",
  "imported.noFilterResults": "\u5f53\u524d\u7b5b\u9009\u6761\u4ef6\u4e0b\u6ca1\u6709\u5339\u914d\u7684\u5df2\u5bfc\u5165\u7b26\u53f7\u3002",
  "imported.copyPart": "\u590d\u5236 LCSC Part",
  "imported.copied": "LCSC Part \u5df2\u590d\u5236\u5230\u526a\u8d34\u677f\u3002",
  "imported.edit": "\u7f16\u8f91",
  "imported.deleteSymbol": "\u5220\u9664",
  "imported.editorTitle": "\u7f16\u8f91\u5df2\u5bfc\u5165\u7b26\u53f7",
  "imported.editorHint": "\u76f4\u63a5\u5728 KiCad \u7b26\u53f7\u5e93\u4e2d\u4fee\u6539 Symbol Name \u6216 LCSC Part\u3002",
  "imported.editorSourceFile": "\u6765\u6e90\u6587\u4ef6:",
  "imported.editorSymbolName": "\u7b26\u53f7\u540d:",
  "imported.editorLcscPart": "LCSC Part:",
  "imported.save": "\u4fdd\u5b58",
  "imported.cancel": "\u53d6\u6d88",
  "imported.deleteConfirm": "\u786e\u5b9a\u8981\u4ece {file} \u5220\u9664 {symbol} \u5417\uff1f\u5982\u679c\u5b58\u5728\u5bf9\u5e94\u7684\u5c01\u88c5\u30013D \u6a21\u578b\u548c checkpoint \u8bb0\u5f55\uff0c\u4e5f\u4f1a\u4e00\u5e76\u5220\u9664\u3002",
  "imported.search": "\u641c\u7d22:",
  "imported.searchPlaceholder": "\u6309 LCSC Part \u6216\u7b26\u53f7\u540d\u7b5b\u9009",
  "imported.total": "\u603b\u6570",
  "imported.filtered": "\u7b5b\u9009\u540e",
  "imported.selected": "\u5df2\u9009",
  "imported.selectFiltered": "\u9009\u62e9\u7b5b\u9009\u7ed3\u679c",
  "imported.clearSelection": "\u6e05\u7a7a\u9009\u62e9",
  "imported.copyParts": "\u590d\u5236 Part",
  "imported.queueParts": "\u52a0\u5165\u961f\u5217",
  "imported.queuePart": "\u5165\u961f",
  "imported.selectionHint": "\u201c\u590d\u5236 Part\u201d\u3001\u201c\u52a0\u5165\u961f\u5217\u201d\u548c\u201c\u5bfc\u51fa Part\u201d\u4f1a\u4f18\u5148\u4f7f\u7528\u5df2\u9009\u6761\u76ee\uff0c\u82e5\u672a\u9009\u4e2d\u5219\u4f7f\u7528\u5f53\u524d\u7b5b\u9009\u7ed3\u679c\u3002",
  "imported.noActionableParts": "\u5f53\u524d\u6ca1\u6709\u53ef\u7528\u4e8e\u6b64\u64cd\u4f5c\u7684 LCSC Part\u3002",
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
  "export.nlbnExportContent": "\u5bfc\u51fa\u5185\u5bb9:",
  "export.nlbnOverwriteExisting": "\u8986\u76d6\u5df2\u5b58\u5728:",
  "export.nlbnAssetSymbol": "Symbol",
  "export.nlbnAssetFootprint": "Footprint",
  "export.nlbnAssetModel3d": "3D Model",
  "export.nlbnSelectAtLeastOne": "\u81f3\u5c11\u542f\u7528\u4e00\u9879\u5bfc\u51fa\u5185\u5bb9\u540e\u624d\u80fd\u6267\u884c nlbn \u5bfc\u51fa\u3002",
  "export.overwriteOn": "\u8986\u76d6: \u5f00",
  "export.overwriteOff": "\u8986\u76d6: \u5173",
  "export.nlbnOverwriteHint": "\u8986\u76d6\u53ea\u5bf9\u5df2\u542f\u7528\u7684\u5bfc\u51fa\u9879\u751f\u6548\uff0c\u5173\u95ed\u67d0\u9879\u5bfc\u51fa\u65f6\u4f1a\u540c\u65f6\u5173\u95ed\u5bf9\u5e94\u7684\u8986\u76d6\u3002",
  "export.nlbnFillColor": "\u7b26\u53f7\u586b\u5145\u989c\u8272:",
  "export.nlbnFillColorHint": "\u53ef\u9009\u3002\u7559\u7a7a\u5219\u4fdd\u6301 nlbn/KiCad \u9ed8\u8ba4\u586b\u5145\u884c\u4e3a\uff0c\u652f\u6301 #RRGGBB \u6216 #RRGGBBAA\u3002",
  "export.nlbnFillColorPlaceholder": "\u793a\u4f8b: #005C8FCC",
  "export.nlbnFillColorClear": "\u6e05\u7a7a",
  "export.nlbnFillColorAuto": "\u4e0d\u8986\u76d6",
  "export.nlbnFillColorInvalid": "\u8bf7\u4f7f\u7528 #RRGGBB \u6216 #RRGGBBAA \u683c\u5f0f\u3002",
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
  "about.tagline": "\u526a\u8d34\u677f\u4e8b\u4ef6\u8ffd\u8e2a\u5668",
  "about.desc": "\u5b9e\u65f6\u76d1\u542c\u526a\u8d34\u677f\uff0c\u6309\u5173\u952e\u5b57\u6216\u6b63\u5219\u63d0\u53d6\u5143\u4ef6 ID\uff0c\u5e76\u901a\u8fc7 nlbn \u6216 npnp \u5bfc\u51fa\u3002",
  "status.keyword": "\u5173\u952e\u5b57:",
  "status.none": "\u65e0",
};

let currentPage: PageName = "monitor";
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

const importedUi: {
  loading: boolean;
  busy: boolean;
  initialized: boolean;
  scannedPath: string;
  items: ImportedSymbol[];
  error: string | null;
  notice: ExportNotice | null;
  query: string;
  selectedKeys: Set<string>;
  editingKey: string | null;
  editDraftSymbolName: string;
  editDraftLcscPart: string;
  editDraftSourceFile: string;
} = {
  loading: false,
  busy: false,
  initialized: false,
  scannedPath: "",
  items: [],
  error: null,
  notice: null,
  query: "",
  selectedKeys: new Set(),
  editingKey: null,
  editDraftSymbolName: "",
  editDraftLcscPart: "",
  editDraftSourceFile: "",
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
  return zhTranslations[key] ?? enTranslations[key] ?? key;
}

function formatMessage(key: string, values: Record<string, string>): string {
  return Object.entries(values).reduce(
    (message, [name, value]) => message.split(`{${name}}`).join(value),
    t(key),
  );
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

function parseOptionalHexColor(value: string): { normalized: string | null; valid: boolean } {
  const trimmed = value.trim();
  if (!trimmed) {
    return { normalized: null, valid: true };
  }

  const match = /^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.exec(trimmed);
  if (!match) {
    return { normalized: null, valid: false };
  }

  return { normalized: `#${match[1].toUpperCase()}`, valid: true };
}

function normalizeImportedLcscPart(value: string): string {
  return value.trim().toUpperCase();
}

function importedRowKey(item: ImportedSymbol): string {
  return `${item.source_file}\u001f${item.symbol_name}\u001f${item.lcsc_part}`;
}

function dedupeImportedParts(items: ImportedSymbol[]): string[] {
  const parts = new Set<string>();
  items.forEach((item) => {
    parts.add(item.lcsc_part);
  });
  return Array.from(parts);
}

function filteredImportedItems(): ImportedSymbol[] {
  const query = importedUi.query.trim().toLowerCase();
  if (!query) {
    return importedUi.items;
  }

  return importedUi.items.filter((item) => {
    return item.lcsc_part.toLowerCase().includes(query) || item.symbol_name.toLowerCase().includes(query);
  });
}

function selectedImportedItems(): ImportedSymbol[] {
  if (importedUi.selectedKeys.size === 0) {
    return [];
  }

  return importedUi.items.filter((item) => importedUi.selectedKeys.has(importedRowKey(item)));
}

function importedItemByKey(key: string | null): ImportedSymbol | null {
  if (!key) {
    return null;
  }
  return importedUi.items.find((item) => importedRowKey(item) === key) ?? null;
}

function activeImportedParts(): string[] {
  const selected = dedupeImportedParts(selectedImportedItems());
  if (selected.length > 0) {
    return selected;
  }
  return dedupeImportedParts(filteredImportedItems());
}

function pruneImportedSelection() {
  const validKeys = new Set(importedUi.items.map((item) => importedRowKey(item)));
  importedUi.selectedKeys = new Set(
    Array.from(importedUi.selectedKeys).filter((key) => validKeys.has(key)),
  );

  if (importedUi.editingKey && !validKeys.has(importedUi.editingKey)) {
    closeImportedEditor();
  }
}

function openImportedEditor(item: ImportedSymbol) {
  importedUi.editingKey = importedRowKey(item);
  importedUi.editDraftSymbolName = item.symbol_name;
  importedUi.editDraftLcscPart = item.lcsc_part;
  importedUi.editDraftSourceFile = item.source_file;
}

function closeImportedEditor() {
  importedUi.editingKey = null;
  importedUi.editDraftSymbolName = "";
  importedUi.editDraftLcscPart = "";
  importedUi.editDraftSourceFile = "";
}

function buildKeyword(): string {
  const parts: string[] = [];
  if (matchFull) parts.push(PATTERN_FULL);
  if (matchQuick) parts.push(PATTERN_QUICK);
  return parts.join("||");
}

function applyStaticTranslations() {
  document.documentElement.lang = "zh-CN";
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n")!;
    el.textContent = t(key);
  });

  document.querySelectorAll("[data-i18n-placeholder]").forEach((el) => {
    const key = el.getAttribute("data-i18n-placeholder")!;
    (el as HTMLInputElement).placeholder = t(key);
  });
  $("btn-toggle-matched").textContent = showMatched ? t("monitor.show") : t("monitor.hide");
  renderImportedPanel();
  rerenderState();
}

function switchPage(pageName: PageName) {
  currentPage = pageName;
  document.querySelectorAll(".page").forEach((p) => p.classList.remove("active"));
  document.querySelectorAll(".nav-item").forEach((n) => n.classList.remove("active"));

  const page = document.getElementById(`page-${pageName}`);
  const nav = document.querySelector(`.nav-item[data-page="${pageName}"]`);
  if (page) page.classList.add("active");
  if (nav) nav.classList.add("active");

  if (pageName === "imported" && !importedUi.initialized) {
    void loadImportedSymbols();
  }
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

function nlbnExportEnabled(state: AppState, field: NlbnExportField): boolean {
  return Boolean(state[field]);
}

function nlbnOverwriteEnabled(state: AppState, field: NlbnOverwriteField): boolean {
  return Boolean(state[field]);
}

function hasAnyNlbnExportEnabled(state: AppState): boolean {
  return nlbnAssetToggles.some((toggle) => nlbnExportEnabled(state, toggle.exportField));
}

function renderNlbnAssetToggles(state: AppState): boolean {
  const anyExportEnabled = hasAnyNlbnExportEnabled(state);

  nlbnAssetToggles.forEach((toggle) => {
    const exportButton = $(toggle.exportButtonId) as HTMLButtonElement;
    const overwriteButton = $(toggle.overwriteButtonId) as HTMLButtonElement;
    const exportEnabled = nlbnExportEnabled(state, toggle.exportField);
    const overwriteEnabled = exportEnabled && nlbnOverwriteEnabled(state, toggle.overwriteField);

    exportButton.classList.toggle("active", exportEnabled);
    exportButton.setAttribute("aria-pressed", String(exportEnabled));

    overwriteButton.classList.toggle("active", overwriteEnabled);
    overwriteButton.disabled = !exportEnabled;
    overwriteButton.setAttribute("aria-pressed", String(overwriteEnabled));
  });

  return anyExportEnabled;
}

function renderExportNotice(tool: ExportTool, derivedNotice: ExportNotice | null = null) {
  const status = $(toolElementId(tool, "status"));
  const notice = exportUi[tool].notice ?? derivedNotice;
  if (!notice) {
    status.textContent = "";
    status.className = "msg msg-warn hidden";
    return;
  }

  status.textContent = notice.message;
  status.className = `msg ${messageClass(notice.kind)}`;
}

function renderExportResult(tool: ExportTool, result: string | null, busy: boolean, derivedNotice: ExportNotice | null = null) {
  const resultBox = $(toolElementId(tool, "result"));
  const promptVisible = tool === "nlbn" && !$("nlbn-not-found").classList.contains("hidden");
  if (!result || busy || exportUi[tool].notice !== null || derivedNotice !== null || promptVisible) {
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
  button.disabled = options.matchedCount === 0 || busy || Boolean(options.buttonDisabled);
  button.textContent = busy ? t("export.running") : t(options.exportLabelKey);

  renderExportProgress(options.tool, busy, t(options.runningLabelKey));
  renderExportNotice(options.tool, options.derivedNotice ?? null);
  renderExportResult(options.tool, options.result, busy, options.derivedNotice ?? null);
}

function syncExportProgressWithState(state: AppState) {
  if (!state.nlbn_running && exportUi.nlbn.progress !== null) {
    exportUi.nlbn.progress = null;
  }
  if (!state.npnp_running && exportUi.npnp.progress !== null) {
    exportUi.npnp.progress = null;
  }
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

function renderNlbnFillColorDraft() {
  const input = $("nlbn-symbol-fill-color-input") as HTMLInputElement;
  const preview = $("nlbn-symbol-fill-color-preview");
  const status = $("nlbn-symbol-fill-color-status");
  const feedback = $("nlbn-symbol-fill-color-feedback");
  const parsed = parseOptionalHexColor(input.value);

  if (!parsed.valid) {
    preview.classList.add("disabled");
    preview.setAttribute("aria-hidden", "true");
    preview.removeAttribute("style");
    status.textContent = t("export.nlbnFillColorAuto");
    feedback.textContent = t("export.nlbnFillColorInvalid");
    feedback.className = "msg msg-error";
    return;
  }

  feedback.textContent = "";
  feedback.className = "msg msg-error hidden";
  if (parsed.normalized) {
    preview.classList.remove("disabled");
    preview.setAttribute("aria-hidden", "false");
    preview.style.background = parsed.normalized;
    status.textContent = parsed.normalized;
  } else {
    preview.classList.add("disabled");
    preview.setAttribute("aria-hidden", "true");
    preview.removeAttribute("style");
    status.textContent = t("export.nlbnFillColorAuto");
  }
}

function renderState(state: AppState) {
  syncOptionalNlbnState(state);
  syncExportProgressWithState(state);

  const kwLabel = t("status.keyword");
  const noneLabel = t("status.none");

  $("status-keyword").textContent = `${kwLabel} ${state.keyword || noneLabel}`;
  $("status-counts").textContent = `H: ${state.history_count} | M: ${state.matched_count}`;
  $("monitor-status").textContent = `${kwLabel} ${state.keyword ? "LCSC" : noneLabel} | H: ${state.history_count} | M: ${state.matched_count}`;
  $("btn-toggle-always-on-top").textContent = state.always_on_top
    ? t("status.alwaysOnTopOn")
    : t("status.alwaysOnTopOff");
  $("btn-toggle-always-on-top").classList.toggle("active", state.always_on_top);

  syncInputValue("nlbn-path-input", state.nlbn_output_path);
  syncInputValue("nlbn-parallel-input", String(state.nlbn_parallel));
  syncInputValue("nlbn-symbol-fill-color-input", state.nlbn_symbol_fill_color ?? "");
  syncInputValue("npnp-path-input", state.npnp_output_path);
  syncInputValue("npnp-library-name-input", state.npnp_library_name);
  syncInputValue("npnp-parallel-input", String(state.npnp_parallel));
  syncInputValue("history-save-path-input", state.history_save_path);
  syncInputValue("matched-save-path-input", state.matched_save_path);
  syncInputValue("imported-parts-save-path-input", state.imported_parts_save_path);

  $("nlbn-terminal-status").textContent = state.nlbn_show_terminal ? t("export.terminalOn") : t("export.terminalOff");
  const nlbnHasExportSelection = renderNlbnAssetToggles(state);
  renderNlbn3dMode();
  renderNlbnFillColorDraft();

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
    buttonDisabled: !nlbnHasExportSelection,
    derivedNotice: nlbnHasExportSelection
      ? null
      : {
          kind: "warn",
          message: t("export.nlbnSelectAtLeastOne"),
        },
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

  renderImportedPanel();
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

function renderImportedList(items: ImportedSymbol[]) {
  const copyLabel = t("monitor.copy");
  const copyTitle = t("imported.copyPart");
  const queueLabel = t("imported.queuePart");
  const editLabel = t("imported.edit");
  const deleteLabel = t("imported.deleteSymbol");
  const container = $("imported-list");
  container.innerHTML = "";

  items.forEach((item) => {
    const key = importedRowKey(item);
    const checked = importedUi.selectedKeys.has(key);
    const row = document.createElement("div");
    row.className = "imported-row";
    row.innerHTML = `
      <label class="imported-select">
        <input type="checkbox" data-select-imported="${escapeAttr(key)}" ${checked ? "checked" : ""} />
      </label>
      <span class="imported-cell imported-part" title="${escapeAttr(item.lcsc_part)}">${escapeHtml(item.lcsc_part)}</span>
      <span class="imported-cell imported-symbol" title="${escapeAttr(item.symbol_name)}">${escapeHtml(item.symbol_name)}</span>
      <span class="imported-actions">
        <button data-queue-imported="${escapeAttr(item.lcsc_part)}" title="${queueLabel}">${queueLabel}</button>
        <button data-copy-imported="${escapeAttr(item.lcsc_part)}" title="${copyTitle}">${copyLabel}</button>
        <button data-edit-imported="${escapeAttr(key)}" title="${editLabel}">${editLabel}</button>
        <button data-delete-imported="${escapeAttr(key)}" title="${deleteLabel}">${deleteLabel}</button>
      </span>`;
    container.appendChild(row);
  });
}

function renderImportedPanel() {
  const filteredItems = filteredImportedItems();
  const selectedItems = selectedImportedItems();
  const activeParts = activeImportedParts();
  const totalParts = dedupeImportedParts(importedUi.items);
  const filteredParts = dedupeImportedParts(filteredItems);
  const selectedParts = dedupeImportedParts(selectedItems);
  const editingItem = importedItemByKey(importedUi.editingKey);
  const count = $("imported-count");
  const path = $("imported-scanned-path");
  const feedback = $("imported-feedback");
  const table = $("imported-table");
  const empty = $("imported-empty");
  const editorCard = $("imported-editor-card");
  const refreshButton = $("btn-refresh-imported") as HTMLButtonElement;
  const browseButton = $("btn-browse-imported-parts-save-path") as HTMLButtonElement;
  const applyButton = $("btn-apply-imported-parts-save-path") as HTMLButtonElement;
  const importButton = $("btn-import-imported-parts") as HTMLButtonElement;
  const exportButton = $("btn-export-imported-parts") as HTMLButtonElement;
  const copyButton = $("btn-copy-imported-parts") as HTMLButtonElement;
  const queueButton = $("btn-queue-imported-parts") as HTMLButtonElement;
  const selectVisibleButton = $("btn-select-imported-visible") as HTMLButtonElement;
  const clearSelectionButton = $("btn-clear-imported-selection") as HTMLButtonElement;
  const saveEditButton = $("btn-save-imported-edit") as HTMLButtonElement;
  const editSymbolInput = $("imported-edit-symbol-name-input") as HTMLInputElement;
  const editLcscInput = $("imported-edit-lcsc-part-input") as HTMLInputElement;
  const cancelEditButtons = [
    $("btn-cancel-imported-edit") as HTMLButtonElement,
    $("btn-cancel-imported-edit-secondary") as HTMLButtonElement,
  ];
  const controlsDisabled = importedUi.loading || importedUi.busy;

  count.textContent = String(totalParts.length);
  $("imported-total-count").textContent = String(totalParts.length);
  $("imported-filtered-count").textContent = String(filteredParts.length);
  $("imported-selected-count").textContent = String(selectedParts.length);
  ($("imported-search-input") as HTMLInputElement).value = importedUi.query;
  refreshButton.disabled = controlsDisabled;
  browseButton.disabled = controlsDisabled;
  applyButton.disabled = controlsDisabled;
  importButton.disabled = controlsDisabled;
  exportButton.disabled = controlsDisabled || activeParts.length === 0;
  copyButton.disabled = controlsDisabled || activeParts.length === 0;
  queueButton.disabled = controlsDisabled || activeParts.length === 0;
  selectVisibleButton.disabled = controlsDisabled || filteredItems.length === 0;
  clearSelectionButton.disabled = controlsDisabled || importedUi.selectedKeys.size === 0;
  saveEditButton.disabled = controlsDisabled || !editingItem;
  editSymbolInput.disabled = controlsDisabled || !editingItem;
  editLcscInput.disabled = controlsDisabled || !editingItem;
  cancelEditButtons.forEach((button) => {
    button.disabled = controlsDisabled;
  });

  const resolvedPath =
    importedUi.scannedPath || lastState?.nlbn_output_path || t("status.none");
  path.textContent = `${t("imported.scannedPath")} ${resolvedPath}`;

  if (editingItem) {
    editorCard.classList.remove("hidden");
    editSymbolInput.value = importedUi.editDraftSymbolName;
    editLcscInput.value = importedUi.editDraftLcscPart;
    $("imported-editor-source-file").textContent = importedUi.editDraftSourceFile;
  } else {
    editorCard.classList.add("hidden");
    editSymbolInput.value = "";
    editLcscInput.value = "";
    $("imported-editor-source-file").textContent = "";
  }

  if (importedUi.notice) {
    feedback.textContent = importedUi.notice.message;
    feedback.className = `msg ${messageClass(importedUi.notice.kind)}`;
  } else if (importedUi.error) {
    feedback.textContent = importedUi.error;
    feedback.className = "msg msg-error";
  } else {
    feedback.textContent = "";
    feedback.className = "msg msg-info hidden";
  }

  if (importedUi.loading) {
    table.classList.add("hidden");
    empty.classList.remove("hidden");
    empty.textContent = t("imported.loading");
    return;
  }

  if (importedUi.error) {
    table.classList.add("hidden");
    empty.classList.add("hidden");
    return;
  }

  if (filteredItems.length > 0) {
    renderImportedList(filteredItems);
    table.classList.remove("hidden");
    empty.classList.add("hidden");
    return;
  }

  table.classList.add("hidden");
  empty.classList.remove("hidden");
  empty.textContent = importedUi.items.length > 0 ? t("imported.noFilterResults") : t("imported.empty");
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
  if (lower.startsWith("saved") || lower.startsWith("exported") || lower.startsWith("queued")) return "success";
  if (lower.includes("failed")) return "error";
  return "warn";
}

function showImportedResult(message: string, kind?: ExportMessageKind) {
  importedUi.notice = {
    kind: kind ?? classifySaveResult(message),
    message,
  };
  renderImportedPanel();
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

async function loadImportedSymbols() {
  importedUi.loading = true;
  importedUi.notice = null;
  renderImportedPanel();

  try {
    const response = await invoke<ImportedSymbolsResponse>("get_imported_symbols");
    importedUi.loading = false;
    importedUi.initialized = true;
    importedUi.scannedPath = response.scanned_path;
    importedUi.items = response.items;
    importedUi.error = null;
    pruneImportedSelection();
  } catch (error) {
    importedUi.loading = false;
    importedUi.initialized = true;
    importedUi.scannedPath = "";
    importedUi.items = [];
    importedUi.error = errorMessage(error);
    importedUi.selectedKeys.clear();
    closeImportedEditor();
  }

  renderImportedPanel();
}

function invalidateImportedSymbols(clearItems = false) {
  importedUi.initialized = false;
  importedUi.scannedPath = "";
  importedUi.error = null;
  if (clearItems) {
    importedUi.items = [];
    importedUi.selectedKeys.clear();
  }
}

let pendingExportConfigWrite: Promise<void> = Promise.resolve();

function queueExportConfigWrite(operation: () => Promise<void>): Promise<void> {
  const run = pendingExportConfigWrite.then(operation, operation);
  pendingExportConfigWrite = run.catch(() => {});
  return run;
}

async function runImportedAction(operation: () => Promise<void>) {
  if (importedUi.loading || importedUi.busy) return;
  importedUi.busy = true;
  renderImportedPanel();
  try {
    await operation();
  } finally {
    importedUi.busy = false;
    renderImportedPanel();
  }
}

async function syncNlbnExportInputs() {
  const path = ($("nlbn-path-input") as HTMLInputElement).value;
  const parallelValue = ($("nlbn-parallel-input") as HTMLInputElement).value;
  const parallel = parsePositiveIntOrFallback(parallelValue, 4);
  const colorInput = ($("nlbn-symbol-fill-color-input") as HTMLInputElement).value;
  const parsedColor = parseOptionalHexColor(colorInput);

  if (!parsedColor.valid) {
    throw new Error(t("export.nlbnFillColorInvalid"));
  }

  await invoke("set_nlbn_path", { path });
  await invoke("set_nlbn_parallel", { parallel });
  await invoke("set_nlbn_symbol_fill_color", { color: parsedColor.normalized });
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

async function saveActiveImportedParts() {
  const parts = activeImportedParts();
  if (parts.length === 0) {
    showImportedResult(t("imported.noActionableParts"), "warn");
    return;
  }

  const path = ($("imported-parts-save-path-input") as HTMLInputElement).value;
  await queueExportConfigWrite(async () => {
    await invoke("set_imported_parts_save_path", { path });
    await refreshState();
  });
  const result = await invoke<string>("save_lcsc_parts", { parts });
  showImportedResult(result);
}

async function queueActiveImportedParts() {
  const parts = activeImportedParts();
  if (parts.length === 0) {
    showImportedResult(t("imported.noActionableParts"), "warn");
    return;
  }

  const result = await invoke<string>("queue_lcsc_parts", { parts });
  showImportedResult(result);
  await refreshState();
}

async function saveImportedEdit() {
  const item = importedItemByKey(importedUi.editingKey);
  if (!item) {
    return;
  }

  const newSymbolName = importedUi.editDraftSymbolName.trim();
  const newLcscPart = normalizeImportedLcscPart(importedUi.editDraftLcscPart);
  const result = await invoke<string>("update_imported_symbol", {
    request: {
      source_file: item.source_file,
      symbol_name: item.symbol_name,
      new_symbol_name: newSymbolName,
      lcsc_part: newLcscPart,
    },
  });

  closeImportedEditor();
  await loadImportedSymbols();
  showImportedResult(result, "success");
}

async function deleteImportedItem(item: ImportedSymbol) {
  const confirmed = window.confirm(
    formatMessage("imported.deleteConfirm", {
      symbol: item.symbol_name,
      file: item.source_file,
    }),
  );
  if (!confirmed) {
    return;
  }

  const result = await invoke<string>("delete_imported_symbol", {
    request: {
      source_file: item.source_file,
      symbol_name: item.symbol_name,
      lcsc_part: item.lcsc_part,
    },
  });

  if (importedUi.editingKey === importedRowKey(item)) {
    closeImportedEditor();
  }
  await loadImportedSymbols();
  showImportedResult(result, "success");
}

window.addEventListener("DOMContentLoaded", async () => {
  applyStaticTranslations();
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
    if (event.payload.tool === "nlbn" && event.payload.success) {
      invalidateImportedSymbols();
      if (currentPage === "imported") {
        await loadImportedSymbols();
      }
    }
  });

  document.querySelectorAll(".nav-item").forEach((item) => {
    item.addEventListener("click", () => {
      const page = item.getAttribute("data-page");
      if (page) switchPage(page as PageName);
    });
  });

  $("btn-collapse").addEventListener("click", () => {
    $("sidebar").classList.toggle("collapsed");
  });

  $("btn-toggle-always-on-top").addEventListener("click", async () => {
    const next = !(lastState?.always_on_top ?? false);
    await invoke("set_window_always_on_top", { alwaysOnTop: next });
    await refreshState();
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
    $("nlbn-not-found").classList.add("hidden");

    if (lastState && !hasAnyNlbnExportEnabled(lastState)) {
      setExportNotice("nlbn", t("export.nlbnSelectAtLeastOne"));
      return;
    }

    try {
      await queueExportConfigWrite(async () => {
        await syncNlbnExportInputs();
        await refreshState();
      });
      await invoke("check_nlbn");
      startExportProgress("nlbn", t("export.nlbnRunning"));
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
        invalidateImportedSymbols(true);
        await refreshState();
      });
      if (currentPage === "imported") {
        await loadImportedSymbols();
      }
    }
  });

  $("btn-apply-nlbn-path").addEventListener("click", async () => {
    const path = ($("nlbn-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_nlbn_path", { path });
      invalidateImportedSymbols(true);
      await refreshState();
    });
    if (currentPage === "imported") {
      await loadImportedSymbols();
    }
  });

  $("btn-toggle-nlbn-terminal").addEventListener("click", async () => {
    await queueExportConfigWrite(async () => {
      await invoke("toggle_nlbn_terminal");
      await refreshState();
    });
  });

  nlbnAssetToggles.forEach((toggle) => {
    $(toggle.exportButtonId).addEventListener("click", async () => {
      const active = $(toggle.exportButtonId).classList.contains("active");
      await queueExportConfigWrite(async () => {
        await invoke(toggle.exportCommand, { enabled: !active });
        if (active) {
          await invoke(toggle.overwriteCommand, { overwrite: false });
        }
        await refreshState();
      });
    });

    $(toggle.overwriteButtonId).addEventListener("click", async () => {
      const button = $(toggle.overwriteButtonId) as HTMLButtonElement;
      if (button.disabled) {
        return;
      }

      const active = button.classList.contains("active");
      await queueExportConfigWrite(async () => {
        await invoke(toggle.overwriteCommand, { overwrite: !active });
        await refreshState();
      });
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

  $("btn-apply-nlbn-symbol-fill-color").addEventListener("click", async () => {
    const input = $("nlbn-symbol-fill-color-input") as HTMLInputElement;
    const parsed = parseOptionalHexColor(input.value);
    renderNlbnFillColorDraft();
    if (!parsed.valid) {
      return;
    }

    await queueExportConfigWrite(async () => {
      await invoke("set_nlbn_symbol_fill_color", { color: parsed.normalized });
      await refreshState();
    });
  });

  $("btn-clear-nlbn-symbol-fill-color").addEventListener("click", async () => {
    const input = $("nlbn-symbol-fill-color-input") as HTMLInputElement;
    input.value = "";
    renderNlbnFillColorDraft();
    await queueExportConfigWrite(async () => {
      await invoke("set_nlbn_symbol_fill_color", { color: null });
      await refreshState();
    });
  });

  $("nlbn-symbol-fill-color-input").addEventListener("input", () => {
    renderNlbnFillColorDraft();
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

  $("btn-refresh-imported").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    importedUi.notice = null;
    await loadImportedSymbols();
  });

  $("imported-search-input").addEventListener("input", (event) => {
    importedUi.query = (event.target as HTMLInputElement).value;
    renderImportedPanel();
  });

  $("btn-select-imported-visible").addEventListener("click", () => {
    filteredImportedItems().forEach((item) => {
      importedUi.selectedKeys.add(importedRowKey(item));
    });
    renderImportedPanel();
  });

  $("btn-clear-imported-selection").addEventListener("click", () => {
    importedUi.selectedKeys.clear();
    renderImportedPanel();
  });

  $("imported-edit-symbol-name-input").addEventListener("input", (event) => {
    importedUi.editDraftSymbolName = (event.target as HTMLInputElement).value;
  });

  $("imported-edit-lcsc-part-input").addEventListener("input", (event) => {
    importedUi.editDraftLcscPart = normalizeImportedLcscPart((event.target as HTMLInputElement).value);
    (event.target as HTMLInputElement).value = importedUi.editDraftLcscPart;
  });

  const cancelImportedEdit = () => {
    if (importedUi.busy) return;
    closeImportedEditor();
    renderImportedPanel();
  };

  $("btn-cancel-imported-edit").addEventListener("click", cancelImportedEdit);
  $("btn-cancel-imported-edit-secondary").addEventListener("click", cancelImportedEdit);

  $("btn-save-imported-edit").addEventListener("click", async () => {
    await runImportedAction(async () => {
      importedUi.notice = null;
      renderImportedPanel();

      try {
        await saveImportedEdit();
      } catch (error) {
        showImportedResult(errorMessage(error), "error");
      }
    });
  });

  $("btn-copy-imported-parts").addEventListener("click", async () => {
    const parts = activeImportedParts();
    if (parts.length === 0) return;
    await invoke("copy_to_clipboard", { text: parts.join("\n") });
    showImportedResult(t("imported.copied"), "success");
  });

  $("btn-queue-imported-parts").addEventListener("click", async () => {
    await runImportedAction(async () => {
      importedUi.notice = null;
      renderImportedPanel();

      try {
        await queueActiveImportedParts();
      } catch (error) {
        showImportedResult(errorMessage(error), "error");
      }
    });
  });

  $("btn-apply-imported-parts-save-path").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    importedUi.notice = null;
    const path = ($("imported-parts-save-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_imported_parts_save_path", { path });
      await refreshState();
    });
  });

  $("btn-browse-imported-parts-save-path").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    importedUi.notice = null;
    const current = ($("imported-parts-save-path-input") as HTMLInputElement).value;
    const selected = await selectSaveFile(t("imported.exportDialog"), current);
    if (selected) {
      ($("imported-parts-save-path-input") as HTMLInputElement).value = selected;
      await queueExportConfigWrite(async () => {
        await invoke("set_imported_parts_save_path", { path: selected });
        await refreshState();
      });
    }
  });

  $("btn-export-imported-parts").addEventListener("click", async () => {
    await runImportedAction(async () => {
      importedUi.notice = null;
      renderImportedPanel();

      try {
        await saveActiveImportedParts();
      } catch (error) {
        showImportedResult(errorMessage(error), "error");
      }
    });
  });

  $("btn-import-imported-parts").addEventListener("click", async () => {
    await runImportedAction(async () => {
      importedUi.notice = null;
      renderImportedPanel();

      try {
        const path = ($("imported-parts-save-path-input") as HTMLInputElement).value;
        await queueExportConfigWrite(async () => {
          await invoke("set_imported_parts_save_path", { path });
          await refreshState();
        });
        const result = await invoke<string>("import_imported_parts");
        const kind: ExportMessageKind =
          result.toLowerCase().includes("failed")
            ? "error"
            : result.startsWith("Imported 0 ") || result.startsWith("No ")
              ? "warn"
              : result.startsWith("Imported ")
                ? "success"
                : "warn";
        showImportedResult(result, kind);
        await refreshState();
      } catch (error) {
        showImportedResult(errorMessage(error), "error");
      }
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

  document.addEventListener("change", (e) => {
    const target = e.target as HTMLElement;
    if (target instanceof HTMLInputElement && target.matches("input[data-select-imported]")) {
      const key = target.getAttribute("data-select-imported");
      if (!key) return;
      if (target.checked) {
        importedUi.selectedKeys.add(key);
      } else {
        importedUi.selectedKeys.delete(key);
      }
      renderImportedPanel();
    }
  });

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

    const importedCopy = target.getAttribute("data-copy-imported");
    if (importedCopy !== null) {
      await invoke("copy_to_clipboard", { text: importedCopy });
      importedUi.notice = { kind: "success", message: t("imported.copied") };
      renderImportedPanel();
      return;
    }

    const importedQueue = target.getAttribute("data-queue-imported");
    if (importedQueue !== null) {
      await runImportedAction(async () => {
        try {
          const result = await invoke<string>("queue_lcsc_parts", { parts: [importedQueue] });
          showImportedResult(result);
          await refreshState();
        } catch (error) {
          showImportedResult(errorMessage(error), "error");
        }
      });
      return;
    }

    const importedEdit = target.getAttribute("data-edit-imported");
    if (importedEdit !== null) {
      const item = importedItemByKey(importedEdit);
      if (!item) {
        return;
      }
      openImportedEditor(item);
      renderImportedPanel();
      return;
    }

    const importedDelete = target.getAttribute("data-delete-imported");
    if (importedDelete !== null) {
      const item = importedItemByKey(importedDelete);
      if (!item) {
        return;
      }
      await runImportedAction(async () => {
        importedUi.notice = null;
        renderImportedPanel();

        try {
          await deleteImportedItem(item);
        } catch (error) {
          showImportedResult(errorMessage(error), "error");
        }
      });
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
