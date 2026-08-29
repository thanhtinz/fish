// The bridge to the Rust backend.
//
// Every call goes through Tauri's `invoke`. There is deliberately no localization logic and no
// stand-in data on this side: outside the desktop shell the interface has no backend, and it says
// so rather than rendering something a screenshot could mistake for the real thing.

import type {
  BuildView,
  CapabilityView,
  DictionaryView,
  GlossView,
  ImportReport,
  LanguageView,
  NodeView,
  ProjectSummary,
  StyleView,
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
  const chosen = await open({
    multiple: false,
    title: "Chọn file JAR của game",
    filters: [{ name: "Java archive", extensions: ["jar"] }],
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
  recentProjects: () => call<ProjectSummary[]>("recent_projects"),

  importJar: (jarPath: string, into: string, name: string | null, targets: string[]) =>
    call<ProjectSummary>("import_jar", { jarPath, into, name, targets }),

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
};
