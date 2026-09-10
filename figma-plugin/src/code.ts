import type { Analysis, ConvertResult, Diagnostic } from "../vendor/svg2vd-wasm/svg2vd_wasm";
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

figma.codegen.on("generate", (event) => {
  if (event.node.type !== "FRAME") return [];
  return withinDeadline(generate(event.node), CODEGEN_DEADLINE_MS);
});

async function generate(frame: FrameNode): Promise<CodegenResult[]> {
  try {
    const source = await frame.exportAsync({ format: "SVG" });
    const result = await convert(source);
    if (result.ok) {
      return [
        {
          title: "Android VectorDrawable",
          language: "PLAINTEXT",
          code: result.xml,
        },
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
          resolve(diagnosticResult("Conversion timed out. Try a simpler frame."));
        }, milliseconds);
      }),
    ]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

function diagnosticResult(message: string, analysis?: Analysis): CodegenResult[] {
  const diagnostics = analysis?.diagnostics ?? [];
  const details = diagnostics.length > 0 ? diagnostics.map(formatDiagnostic).join("\n") : message;
  return [
    {
      title: "svg2vd diagnostics",
      language: "PLAINTEXT",
      code: details,
    },
  ];
}

function formatDiagnostic(diagnostic: Diagnostic): string {
  const location = diagnostic.location
    ? ` (${diagnostic.location.element} at ${diagnostic.location.line}:${diagnostic.location.column})`
    : "";
  const suggestion = diagnostic.suggestion ? `\n  Suggestion: ${diagnostic.suggestion}` : "";
  return `[${diagnostic.code}] ${diagnostic.message}${location}${suggestion}`;
}
