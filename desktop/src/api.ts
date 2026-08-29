// The bridge to the Rust backend.
//
// Every call goes through Tauri's `invoke`. There is deliberately no localization logic and no
// stand-in data on this side: outside the desktop shell the interface has no backend, and it says
// so rather than rendering something that a screenshot could mistake for the real thing.

import type {
  BuildView,
  CapabilityView,
  NodeView,
  ProjectSummary,
} from "./types";

export const isDesktop = "__TAURI_INTERNALS__" in window;

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop) {
    throw new Error(
      "no backend: this interface runs inside the TJLocalizer desktop shell",
    );
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

export async function pickJar(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const chosen = await open({
    multiple: false,
    filters: [{ name: "Java archive", extensions: ["jar"] }],
  });
  return typeof chosen === "string" ? chosen : null;
}

export async function pickFolder(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const chosen = await open({ directory: true, multiple: false });
  return typeof chosen === "string" ? chosen : null;
}

export const api = {
  recentProjects: () => call<ProjectSummary[]>("recent_projects"),

  importJar: (jarPath: string, into: string, name: string | null) =>
    call<ProjectSummary>("import_jar", { jarPath, into, name }),

  openProject: (path: string) => call<ProjectSummary>("open_project", { path }),

  projectSummary: (path: string) => call<ProjectSummary>("project_summary", { path }),

  analyze: (path: string) => call<CapabilityView[]>("analyze", { path }),

  capabilities: (path: string) => call<CapabilityView[]>("capabilities", { path }),

  extract: (path: string) => call<number>("extract", { path }),

  nodes: (path: string) => call<NodeView[]>("nodes", { path }),

  setTranslation: (path: string, nodeId: string, target: string) =>
    call<void>("set_translation", { path, nodeId, target }),

  suggestAll: (path: string, fuzzyThreshold: number) =>
    call<number>("suggest_all", { path, fuzzyThreshold }),

  applySafe: (path: string) => call<number>("apply_safe", { path }),

  learn: (path: string) => call<number>("learn", { path }),

  build: (path: string) => call<BuildView>("build", { path }),

  builds: (path: string) => call<BuildView[]>("builds", { path }),

  rollback: (path: string, revision: number) =>
    call<BuildView>("rollback", { path, revision }),

  setBranding: (path: string, enabled: boolean) =>
    call<ProjectSummary>("set_branding", { path, enabled }),

  setLocalization: (path: string, targetLanguage: string, styleProfile: string) =>
    call<ProjectSummary>("set_localization", { path, targetLanguage, styleProfile }),

  outputPath: (path: string) => call<string | null>("output_path", { path }),
};
