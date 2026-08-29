// Mirrors the view models in crates/tjlocalizer-desktop/src/state.rs.
// Kept by hand rather than generated: the surface is small, and a hand-written file is one a
// reviewer can read against the Rust without a build step in between.

export interface ProjectSummary {
  path: string;
  name: string;
  targetLanguage: string;
  styleProfile: string;
  sourceSha256: string;
  revision: number;
  brandingEnabled: boolean;
  needsAnalyze: boolean;
  needsExtract: boolean;
  nodeCount: number;
  translatableCount: number;
  approvedCount: number;
  buildCount: number;
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
