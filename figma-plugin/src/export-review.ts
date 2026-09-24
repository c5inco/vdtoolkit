import type { ConvertResult, Diagnostic } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";
import { blockingDiagnostics, capitalize, warningDiagnostics } from "./diagnostics";
import type { ExportCandidate, ExportKind } from "./messages";
import { uniqueResourceNames } from "./resource-names";

// Android's recommended vector icon limit. The dialog scales larger layers down to fit,
// keeping proportions, instead of refusing them; the Code panel keeps Figma's size.
export const MAX_EXPORT_DP = 200;

// Android's warning for drawables over MAX_EXPORT_DP. The dialog always resizes those, so
// the warning is never a reason a layer can't be exported.
const LARGE_DIMENSIONS = "VDT016";

export interface Issue {
  message: string;
  code?: string;
  suggestion?: string;
}

export interface ReadyRow {
  status: "ready";
  nodeId: string;
  name: string;
  preview?: Uint8Array;
  // The layer's size in Figma, and the drawable's size after capping at MAX_EXPORT_DP.
  width?: number;
  height?: number;
  exportWidth: number;
  exportHeight: number;
  // True when the drawable was scaled down to fit MAX_EXPORT_DP.
  resized: boolean;
  fileName: string;
  xml: string;
  warnings: Issue[];
}

export interface BlockedRow {
  status: "blocked";
  nodeId: string;
  name: string;
  preview?: Uint8Array;
  width?: number;
  height?: number;
  issues: Issue[];
}

export type ReviewRow = ReadyRow | BlockedRow;

// Converts every candidate up front so the dialog can say exactly which layers will export.
// `convert` is expected to cap drawables at MAX_EXPORT_DP.
export function reviewCandidates(
  candidates: ExportCandidate[],
  convert: (source: Uint8Array) => ConvertResult,
  kind: ExportKind = "drawable",
): ReviewRow[] {
  const results = candidates.map((candidate) =>
    candidate.source && !candidate.skipReason ? convert(candidate.source) : undefined,
  );
  const readyNames = uniqueResourceNames(
    candidates.filter((_, index) => results[index]?.ok).map((candidate) => candidate.name),
  );

  let readyIndex = 0;
  return candidates.map((candidate, index): ReviewRow => {
    const { nodeId, name, source: preview, width, height } = candidate;
    const result = results[index];
    if (!result) {
      const message = candidate.skipReason ?? "Figma didn't export this layer.";
      return { status: "blocked", nodeId, name, preview, width, height, issues: [{ message }] };
    }
    if (result.ok) {
      return {
        status: "ready",
        nodeId,
        name,
        preview,
        width,
        height,
        exportWidth: result.analysis.metrics.width,
        exportHeight: result.analysis.metrics.height,
        resized: kind === "drawable" && wasResized(candidate, result.analysis.metrics),
        fileName: `${readyNames[readyIndex++]}.xml`,
        xml: result.xml,
        warnings: warningDiagnostics(result.analysis.diagnostics).map(toIssue),
      };
    }
    // A failed conversion never reaches resizing, so its analysis still carries the size warning.
    const diagnostics = blockingDiagnostics(
      (result.error.analysis?.diagnostics ?? []).filter((diagnostic) => diagnostic.code !== LARGE_DIMENSIONS),
    );
    const issues =
      diagnostics.length > 0 ? oncePerCode(diagnostics).map(toIssue) : [{ message: capitalize(result.error.message) }];
    return { status: "blocked", nodeId, name, preview, width, height, issues };
  });
}

// Case-insensitive match on what a row shows: the layer name and, once ready, its file name.
export function matchesFilter(row: ReviewRow, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (needle === "") return true;
  const text = row.status === "ready" ? `${row.name}\n${row.fileName}` : row.name;
  return text.toLowerCase().includes(needle);
}

// Judged from the converter's output rather than a separate size check, so the note
// appears exactly when resizing happened. Resizing always shrinks the longest side.
function wasResized({ width, height }: ExportCandidate, exported: { width: number; height: number }): boolean {
  if (width === undefined || height === undefined) return false;
  return Math.max(exported.width, exported.height) < Math.max(width, height);
}

// One cause often yields several wordings under the same code; a checklist needs each fix once.
function oncePerCode(diagnostics: Diagnostic[]): Diagnostic[] {
  const seen = new Set<string>();
  return diagnostics.filter((diagnostic) => !seen.has(diagnostic.code) && seen.add(diagnostic.code));
}

function toIssue(diagnostic: Diagnostic): Issue {
  return {
    message: capitalize(diagnostic.message),
    code: diagnostic.code,
    suggestion: diagnostic.suggestion,
  };
}
