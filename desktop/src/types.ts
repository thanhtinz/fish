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
