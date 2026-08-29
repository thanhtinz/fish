// Mirrors the view models in crates/tjlocalizer-desktop/src/state.rs.
// Kept by hand rather than generated: the surface is small, and a hand-written file is one a
// reviewer can read against the Rust without a build step in between.

export interface TargetView {
  tag: string;
  name: string;
  styleProfile: string;
  enabled: boolean;
  approvedCount: number;
  buildCount: number;
  outputPath: string | null;
}

export interface ProjectSummary {
  path: string;
  name: string;
  sourceLanguage: string;
  sourceLanguageName: string;
  sourceLanguageDetected: boolean;
  targets: TargetView[];
  sourceSha256: string;
  revision: number;
  brandingEnabled: boolean;
  needsAnalyze: boolean;
  needsExtract: boolean;
  nodeCount: number;
  translatableCount: number;
}

export interface RecentView {
  path: string;
  summary: ProjectSummary | null;
  error: string | null;
}

export interface LanguageView {
  tag: string;
  name: string;
  script: string;
}

export interface StyleView {
  id: string;
  language: string;
  description: string;
  firstPerson: string;
  secondPerson: string;
}

export interface GlossTerm {
  source: string;
  target: string;
  domain: string;
}

export interface GlossView {
  text: string;
  completeness: "complete" | "partial" | "none";
  confidence: number;
  engine: string;
  terms: GlossTerm[];
  unresolved: string[];
  notes: string[];
}

export interface DictionaryView {
  from: string;
  to: string;
  fromName: string;
  toName: string;
  entries: number;
}

export interface ImportReport {
  applied: number;
  unchanged: number;
  unknown: number;
  malformed: number[];
}

export interface Location {
  kind: "class" | "resource";
  file: string;
  detail: string;
}

export type CandidateOrigin = "memory" | "memory-fuzzy" | "glossary";

export interface CandidateView {
  target: string;
  origin: CandidateOrigin;
  score: number | null;
  autoApprovable: boolean;
}

export interface IssueView {
  code: string;
  detail: string;
  blocking: boolean;
}

export interface NodeView {
  id: string;
  source: string;
  target: string | null;
  context: string;
  translatable: boolean;
  location: Location;
  placeholders: string[];
  sourceEncoding: string | null;
  candidate: CandidateView | null;
  issues: IssueView[];
  sourceWidth?: number;
  targetWidth?: number;
}

export interface FindingView {
  severity: "error" | "warning";
  check: string;
  detail: string;
}

export interface BuildView {
  revision: number;
  language: string;
  profileRevision: number;
  literalsPatched: number;
  resourcesPatched: number;
  translationsApplied: number;
  outputSha256: string;
  ok: boolean;
  findings: FindingView[];
}

export interface CapabilityView {
  id: string;
  confidence: number;
  evidence: string[];
}

export interface EngineKindView {
  id: string;
  defaultEndpoint: string;
  takesInstructions: boolean;
}

export interface EngineView {
  configured: boolean;
  enabled: boolean;
  kind: string;
  endpoint: string;
  model: string | null;
  hasKey: boolean;
  kinds: EngineKindView[];
}

export interface EnginePreview {
  url: string;
  instructions: string;
  body: string;
}

export interface GridView {
  cellWidth: number;
  cellHeight: number;
  columns: number;
  rows: number;
}

export interface GridSuggestionView extends GridView {
  fit: number;
  capacity: number;
}

export interface SheetCandidateView {
  entry: string;
  width: number;
  height: number;
  inkShare: number;
  colours: number;
  grids: GridSuggestionView[];
  image: string;
}

export interface FontView {
  declared: boolean;
  entry: string;
  deviceFont: boolean;
  grid: GridView | null;
  order: string;
  markLibrary: string | null;
  marksFrom: string | null;
  covered: number;
  required: number;
  missing: string;
  composable: number;
  problem: string | null;
}

export interface FontFitView {
  path: string;
  name: string;
  fromTypeface: number;
  composed: number;
  share: number;
  chosen: boolean;
}

export interface FontScan {
  found: number;
  covering: number;
  measured: number;
  fonts: FontFitView[];
}

export interface CompositionView {
  path: string;
  added: string;
  skipped: { reason: string; letters: string }[];
  fromTypeface: number;
  typeface: string | null;
  image: string;
}

export interface RuleView {
  id: string;
  description: string;
  enabled: boolean;
  ready: boolean;
  effects: string[];
  unmet: string[];
}

export interface AlternativeView {
  text: string;
  width: number | null;
  why: string;
}
