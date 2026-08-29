// The bridge to the Rust backend.
//
// Every call goes through Tauri's `invoke`. There is deliberately no localization logic and no
// stand-in data on this side: outside the desktop shell the interface has no backend, and it says
// so rather than rendering something a screenshot could mistake for the real thing.

import type {
  AlternativeView,
  AnalystView,
  BuildView,
  CapabilityView,
  CompositionView,
  DictionaryView,
  EnginePreview,
  EngineView,
  FontScan,
  FontView,
  GlossView,
  GridView,
  ImageAssetView,
  ContextView,
  FontLookupView,
  RegressionView,
  PluginsView,
  ReadingView,
  ImportReport,
  IngestView,
  InspectionView,
  LanguageView,
  NodeView,
  PatchPlanView,
  ProjectSummary,
  RecentView,
  ReviewNoteView,
  RuleView,
  ScanPreview,
  SheetCandidateView,
  StyleView,
  SuggestionView,
} from "./types";

export const isDesktop = "__TAURI_INTERNALS__" in window;

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop) {
    throw new Error("no backend: this interface runs inside the TJLocalizer desktop shell");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

export async function pickJar(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  // All of these are ZIP archives underneath, and which one it is is worked out from what is
  // inside rather than from the extension - so the filter is a convenience, not a rule.
  const chosen = await open({
    multiple: false,
    title: "Chọn file game (.jar, .apk, .ipa, .zip)",
    filters: [
      { name: "Game package", extensions: ["jar", "apk", "ipa", "zip"] },
      { name: "Mọi file", extensions: ["*"] },
    ],
  });
  return typeof chosen === "string" ? chosen : null;
}

export async function pickFolder(title: string): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const chosen = await open({ directory: true, multiple: false, title });
  return typeof chosen === "string" ? chosen : null;
}

export async function pickFile(
  title: string,
  name: string,
  extensions: string[],
): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const chosen = await open({ multiple: false, title, filters: [{ name, extensions }] });
  return typeof chosen === "string" ? chosen : null;
}

/** A save dialog: where the finished file should go is the user's business, not the tool's. */
export async function pickSavePath(
  title: string,
  defaultPath: string,
  name: string,
  extensions: string[],
): Promise<string | null> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const chosen = await save({ title, defaultPath, filters: [{ name, extensions }] });
  return typeof chosen === "string" ? chosen : null;
}

export const api = {
  recentProjects: () => call<RecentView[]>("recent_projects"),

  importJar: (jarPath: string, into: string, name: string | null, targets: string[]) =>
    call<ProjectSummary>("import_jar", { jarPath, into, name, targets }),

  importTree: (gamePath: string, into: string, name: string | null, targets: string[]) =>
    call<IngestView>("import_tree", { gamePath, into, name, targets }),

  openProject: (path: string) => call<ProjectSummary>("open_project", { path }),
  projectSummary: (path: string) => call<ProjectSummary>("project_summary", { path }),

  analyze: (path: string) => call<CapabilityView[]>("analyze", { path }),
  capabilities: (path: string) => call<CapabilityView[]>("capabilities", { path }),
  extract: (path: string) => call<number>("extract", { path }),

  nodes: (path: string, language: string) => call<NodeView[]>("nodes", { path, language }),

  gloss: (path: string, language: string, nodeId: string) =>
    call<GlossView | null>("gloss", { path, language, nodeId }),

  setTranslation: (path: string, language: string, nodeId: string, target: string) =>
    call<void>("set_translation", { path, language, nodeId, target }),

  suggestAll: (path: string, language: string, fuzzyThreshold: number) =>
    call<number>("suggest_all", { path, language, fuzzyThreshold }),

  applySafe: (path: string, language: string) => call<number>("apply_safe", { path, language }),
  learn: (path: string, language: string) => call<number>("learn", { path, language }),

  build: (path: string, language: string) => call<BuildView>("build", { path, language }),
  buildAll: (path: string) => call<BuildView[]>("build_all", { path }),
  builds: (path: string, language: string) => call<BuildView[]>("builds", { path, language }),

  rollback: (path: string, language: string, revision: number) =>
    call<BuildView>("rollback", { path, language, revision }),

  exportBuild: (path: string, language: string, destination: string, overwrite: boolean) =>
    call<string>("export_build", { path, language, destination, overwrite }),

  exportTranslations: (
    path: string,
    language: string,
    destination: string,
    onlyUntranslated: boolean,
  ) => call<number>("export_translations", { path, language, destination, onlyUntranslated }),

  importTranslations: (path: string, language: string, source: string) =>
    call<ImportReport>("import_translations", { path, language, source }),

  importDictionary: (path: string, source: string) =>
    call<number>("import_dictionary", { path, source }),

  addTarget: (path: string, language: string, styleProfile: string | null) =>
    call<ProjectSummary>("add_target", { path, language, styleProfile }),

  removeTarget: (path: string, language: string) =>
    call<ProjectSummary>("remove_target", { path, language }),

  setStyle: (path: string, language: string, styleProfile: string) =>
    call<ProjectSummary>("set_style", { path, language, styleProfile }),

  setSourceLanguage: (path: string, language: string) =>
    call<ProjectSummary>("set_source_language", { path, language }),

  setBranding: (path: string, enabled: boolean) =>
    call<ProjectSummary>("set_branding", { path, enabled }),

  languages: () => call<LanguageView[]>("languages"),
  styles: (language: string | null) => call<StyleView[]>("styles", { language }),
  dictionaries: (path: string | null) => call<DictionaryView[]>("dictionaries", { path }),

  engine: (path: string) => call<EngineView>("engine", { path }),

  setEngine: (
    path: string,
    kind: string,
    endpoint: string,
    model: string | null,
    enabled: boolean,
  ) => call<EngineView>("set_engine", { path, kind, endpoint, model, enabled }),

  setEngineKey: (path: string, key: string) => call<EngineView>("set_engine_key", { path, key }),

  enginePreview: (path: string, language: string, text: string) =>
    call<EnginePreview>("engine_preview", { path, language, text }),

  engineTranslate: (path: string, language: string, nodeId: string) =>
    call<GlossView | null>("engine_translate", { path, language, nodeId }),

  fontStatus: (path: string) => call<FontView>("font_status", { path }),
  fontCandidates: (path: string) => call<SheetCandidateView[]>("font_candidates", { path }),

  setFontSheet: (path: string, entry: string, grid: GridView, order: string | null) =>
    call<FontView>("set_font_sheet", { path, entry, grid, order }),

  setDeviceFont: (path: string) => call<FontView>("set_device_font", { path }),
  clearFont: (path: string) => call<FontView>("clear_font", { path }),

  scanFontLibrary: (path: string, directory: string, limit: number | null) =>
    call<FontScan>("scan_font_library", { path, directory, limit }),

  setMarksFont: (path: string, font: string | null) =>
    call<FontView>("set_marks_font", { path, font }),

  composeFont: (path: string) => call<CompositionView>("compose_font", { path }),

  fontPreview: (path: string, text: string | null, scale: number | null) =>
    call<string>("font_preview", { path, text, scale }),

  shorter: (path: string, language: string, nodeId: string) =>
    call<AlternativeView[]>("shorter", { path, language, nodeId }),

  renderText: (path: string, text: string, scale: number | null) =>
    call<string | null>("render_text", { path, text, scale }),

  proofSheet: (path: string, language: string, scale: number | null) =>
    call<string | null>("proof_sheet", { path, language, scale }),

  fontLookupCandidates: (path: string) =>
    call<FontLookupView[]>("font_lookup_candidates", { path }),

  visualRegression: (path: string, language: string, scale: number | null) =>
    call<RegressionView>("visual_regression", { path, language, scale }),

  acceptBaseline: (path: string, language: string, scale: number | null) =>
    call<boolean>("accept_baseline", { path, language, scale }),

  imageAssets: (path: string) => call<ImageAssetView[]>("image_assets", { path }),

  readTextAssets: (path: string, entries: string[]) =>
    call<ReadingView[]>("read_text_assets", { path, entries }),

  markTextAsset: (
    path: string,
    entry: string,
    says: string | null,
    replacement: string | null,
  ) => call<ImageAssetView[]>("mark_text_asset", { path, entry, says, replacement }),

  unmarkTextAsset: (path: string, entry: string) =>
    call<ImageAssetView[]>("unmark_text_asset", { path, entry }),

  plugins: (path: string) => call<PluginsView>("plugins", { path }),

  context: (path: string) => call<ContextView>("context", { path }),

  rules: (path: string) => call<RuleView[]>("rules", { path }),
  writeFontInstallRule: (path: string) => call<RuleView[]>("write_font_install_rule", { path }),

  setRuleEnabled: (path: string, id: string, enabled: boolean) =>
    call<RuleView[]>("set_rule_enabled", { path, id, enabled }),

  removeRule: (path: string, id: string) => call<RuleView[]>("remove_rule", { path, id }),

  planPatch: (path: string, language: string, game: string) =>
    call<PatchPlanView>("plan_patch", { path, language, game }),

  applyPatch: (path: string, language: string, game: string) =>
    call<string[]>("apply_patch", { path, language, game }),

  analyst: (path: string) => call<AnalystView>("analyst", { path }),

  setAnalyst: (path: string, model: string, enabled: boolean) =>
    call<AnalystView>("set_analyst", { path, model, enabled }),

  setAnalystKey: (path: string, key: string) =>
    call<AnalystView>("set_analyst_key", { path, key }),

  scanPreview: (path: string) => call<ScanPreview>("scan_preview", { path }),

  scan: (path: string) => call<SuggestionView[]>("scan", { path }),

  suggestions: (path: string) => call<SuggestionView[]>("suggestions", { path }),

  inspectEntry: (path: string, entry: string) =>
    call<InspectionView>("inspect_entry", { path, entry }),

  reviewLanguage: (path: string, language: string, limit: number) =>
    call<ReviewNoteView[]>("review_language", { path, language, limit }),
};
