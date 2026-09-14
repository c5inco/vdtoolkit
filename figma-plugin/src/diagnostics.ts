import type { Diagnostic } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";

// Info diagnostics (e.g. SVGVD011 "normalization required") are noise next to real blockers.
export function blockingDiagnostics(diagnostics: Diagnostic[]): Diagnostic[] {
  const blocking = diagnostics.filter((diagnostic) => diagnostic.severity !== "info");
  return uniqueDiagnostics(blocking.length > 0 ? blocking : diagnostics);
}

// Conversion succeeded, but Android would still flag the output (e.g. SVGVD016 for icons over 200dp).
export function warningDiagnostics(diagnostics: Diagnostic[]): Diagnostic[] {
  return uniqueDiagnostics(diagnostics.filter((diagnostic) => diagnostic.severity === "warning"));
}

function uniqueDiagnostics(diagnostics: Diagnostic[]): Diagnostic[] {
  const seen = new Set<string>();
  return diagnostics.filter((diagnostic) => !seen.has(diagnostic.message) && seen.add(diagnostic.message));
}

export function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
