import type { Analysis, ConvertResult, Diagnostic } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";
import type { ConvertRequest, ConvertResponse } from "./messages";

declare const __html__: string;

const CODEGEN_DEADLINE_MS = 10_000;
const IFRAME_TIMEOUT_MS = 8_000;
let requestSequence = 0;
const pending = new Map<
  string,
  {
    resolve: (result: ConvertResult) => void;
    reject: (error: Error) => void;
    timer: ReturnType<typeof setTimeout>;
  }
>();

figma.showUI(__html__, { visible: false });

figma.ui.onmessage = (message: ConvertResponse) => {
  if (message.type !== "converted" || typeof message.id !== "string") return;
  const request = pending.get(message.id);
  if (!request) return;
  pending.delete(message.id);
  clearTimeout(request.timer);
  request.resolve(message.result);
};

type ConvertibleNode = FrameNode | ComponentNode | InstanceNode;

figma.codegen.on("generate", (event) => {
  if (!isConvertible(event.node)) return [];
  return withinDeadline(generate(event.node), CODEGEN_DEADLINE_MS);
});

// Component sets are excluded: exporting one would stack every variant into a single drawable.
function isConvertible(node: SceneNode): node is ConvertibleNode {
  return node.type === "FRAME" || node.type === "COMPONENT" || node.type === "INSTANCE";
}

async function generate(node: ConvertibleNode): Promise<CodegenResult[]> {
  try {
    const source = await node.exportAsync({ format: "SVG" });
    const result = await convert(source);
    if (result.ok) {
      // Large drawables are kept at Figma's size; SVGVD016 surfaces below the XML instead.
      return [
        {
          title: "Android Vector Drawable",
          // CodegenResult has no XML language; HTML highlighting handles VectorDrawable markup.
          language: "HTML",
          code: result.xml,
        },
        ...warningResult(result.analysis),
      ];
    }
    return diagnosticResult(result.error.message, result.error.analysis);
  } catch (error) {
    return diagnosticResult(error instanceof Error ? error.message : String(error));
  }
}

function convert(source: Uint8Array): Promise<ConvertResult> {
  const id = `${Date.now()}-${requestSequence++}`;
  const request: ConvertRequest = { type: "convert", id, source };
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error("The WebAssembly converter did not respond in time."));
    }, IFRAME_TIMEOUT_MS);
    pending.set(id, { resolve, reject, timer });
    try {
      figma.ui.postMessage(request);
    } catch (error) {
      clearTimeout(timer);
      pending.delete(id);
      reject(error);
    }
  });
}

async function withinDeadline(
  operation: Promise<CodegenResult[]>,
  milliseconds: number,
): Promise<CodegenResult[]> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      operation,
      new Promise<CodegenResult[]>((resolve) => {
        timer = setTimeout(() => {
          resolve(diagnosticResult("Conversion timed out. Try a simpler selection."));
        }, milliseconds);
      }),
    ]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

// The Code panel neither wraps lines nor renders markup, so wrap by hand to avoid horizontal scrolling.
const WRAP_COLUMNS = 52;

function diagnosticResult(message: string, analysis?: Analysis): CodegenResult[] {
  const diagnostics = analysis?.diagnostics ?? [];
  // Info diagnostics (e.g. SVGVD011 "normalization required") are noise next to real blockers.
  const blocking = diagnostics.filter((diagnostic) => diagnostic.severity !== "info");
  const shown = blocking.length > 0 ? blocking : diagnostics;
  return [
    {
      title: "Can't convert to Vector Drawable",
      language: "PLAINTEXT",
      code: shown.length > 0 ? formatDiagnostics(shown) : wrap(capitalize(message), ""),
    },
  ];
}

// Conversion succeeded, but Android would still flag the output (e.g. SVGVD016 for icons over 200dp).
function warningResult(analysis: Analysis): CodegenResult[] {
  const warnings = analysis.diagnostics.filter((diagnostic) => diagnostic.severity === "warning");
  if (warnings.length === 0) return [];
  return [{ title: "Warnings", language: "PLAINTEXT", code: formatDiagnostics(warnings) }];
}

function formatDiagnostics(diagnostics: Diagnostic[]): string {
  const seen = new Set<string>();
  return diagnostics
    .filter((diagnostic) => !seen.has(diagnostic.message) && seen.add(diagnostic.message))
    .map(formatDiagnostic)
    .join("\n\n");
}

// Source locations are omitted: they point into Figma's SVG export, which the user never sees.
function formatDiagnostic(diagnostic: Diagnostic): string {
  const summary = wrap(`• ${capitalize(diagnostic.message)} (${diagnostic.code})`, "  ");
  return diagnostic.suggestion ? `${summary}\n${wrap(`→ ${diagnostic.suggestion}`, "  ", "  ")}` : summary;
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function wrap(text: string, continuation: string, first = ""): string {
  const lines: string[] = [];
  let line = first;
  for (const word of text.split(/\s+/)) {
    const prefix = lines.length === 0 ? first : continuation;
    if (line.length > prefix.length && line.length + 1 + word.length > WRAP_COLUMNS) {
      lines.push(line);
      line = continuation + word;
    } else {
      line = line.length > prefix.length ? `${line} ${word}` : line + word;
    }
  }
  lines.push(line);
  return lines.join("\n");
}
