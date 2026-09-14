import type { ConvertResult, Diagnostic } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";
import { blockingDiagnostics, capitalize, warningDiagnostics } from "./diagnostics";
import type { ExportCandidate } from "./messages";
import { uniqueResourceNames } from "./resource-names";

export interface Issue {
  message: string;
  code?: string;
  suggestion?: string;
}

export interface ReadyRow {
  status: "ready";
  nodeId: string;
  name: string;
  fileName: string;
  xml: string;
  warnings: Issue[];
}

export interface BlockedRow {
  status: "blocked";
  nodeId: string;
  name: string;
  issues: Issue[];
}

export type ReviewRow = ReadyRow | BlockedRow;

// Converts every candidate up front so the dialog can say exactly which layers will export.
export function reviewCandidates(
  candidates: ExportCandidate[],
  convert: (source: Uint8Array) => ConvertResult,
): ReviewRow[] {
  const results = candidates.map((candidate) =>
    candidate.source ? convert(candidate.source) : undefined,
  );
  const readyNames = uniqueResourceNames(
    candidates.filter((_, index) => results[index]?.ok).map((candidate) => candidate.name),
  );

  let readyIndex = 0;
  return candidates.map((candidate, index): ReviewRow => {
    const { nodeId, name } = candidate;
    const result = results[index];
    if (!result) {
      const message = candidate.skipReason ?? "Figma didn't export this layer.";
      return { status: "blocked", nodeId, name, issues: [{ message }] };
    }
    if (result.ok) {
      return {
        status: "ready",
        nodeId,
        name,
        fileName: `${readyNames[readyIndex++]}.xml`,
        xml: result.xml,
        warnings: warningDiagnostics(result.analysis.diagnostics).map(toIssue),
      };
    }
    const diagnostics = blockingDiagnostics(result.error.analysis?.diagnostics ?? []);
    const issues =
      diagnostics.length > 0 ? oncePerCode(diagnostics).map(toIssue) : [{ message: capitalize(result.error.message) }];
    return { status: "blocked", nodeId, name, issues };
  });
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
