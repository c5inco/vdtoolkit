import type { Analysis, ConvertResult, Diagnostic } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";
import { blockingDiagnostics, capitalize, warningDiagnostics } from "./diagnostics";
import type { ConvertRequest, ExportCandidate, ExportKind, ReviewRequest, SandboxMessage } from "./messages";

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

type ConvertibleNode = FrameNode | ComponentNode | InstanceNode;

// The layers the export dialog opened with. Refresh re-checks these rather than the
// current selection, because fixing a layer usually means selecting just that layer.
let reviewedNodeIds: string[] = [];
let exportKind: ExportKind = "drawable";

figma.ui.onmessage = (message: SandboxMessage) => {
  switch (message?.type) {
    case "converted":
      return settle(message.id, message.result);
    case "focus":
      return focus(message.nodeId);
    case "exported":
      return figma.notify(
        exportKind === "notification"
          ? message.count === 1
            ? "Exported 1 notification icon"
            : `Exported ${message.count} notification icons`
          : message.count === 1
            ? "Exported 1 Vector Drawable"
            : `Exported ${message.count} Vector Drawables`,
      );
    case "close":
      return figma.closePlugin();
    case "refresh":
      return refreshReview();
  }
};

// Dev Mode's Code panel runs the plugin in "codegen" mode; the menu command runs it normally.
if (figma.mode === "codegen") {
  figma.showUI(__html__, { visible: false });
  figma.codegen.on("generate", (event) => {
    if (!isConvertible(event.node)) return [];
    return withinDeadline(generate(event.node), CODEGEN_DEADLINE_MS);
  });
} else {
  exportKind = figma.command === "export-notification-icons" ? "notification" : "drawable";
  void openExportDialog();
}

// Component sets are excluded: exporting one would stack every variant into a single drawable.
function isConvertible(node: SceneNode): node is ConvertibleNode {
  return node.type === "FRAME" || node.type === "COMPONENT" || node.type === "INSTANCE";
}

async function openExportDialog(): Promise<void> {
  const selection = figma.currentPage.selection;
  if (selection.length === 0) {
    figma.notify(
      exportKind === "notification"
        ? "Select one or more frames to export as notification icons."
        : "Select one or more frames to export as Vector Drawables.",
    );
    figma.closePlugin();
    return;
  }
  reviewedNodeIds = selection.map((node) => node.id);
  figma.showUI(__html__, {
    width: 400,
    height: 480,
    title: exportKind === "notification" ? "Export Notification Icons" : "Export Vector Drawables",
    themeColors: true,
  });
  await postReview(selection);
}

// Layers deleted since the dialog opened are dropped: there is nothing left to fix or export.
async function refreshReview(): Promise<void> {
  const nodes = await Promise.all(reviewedNodeIds.map((id) => figma.getNodeByIdAsync(id)));
  await postReview(nodes.filter(isSceneNode));
}

async function postReview(nodes: readonly SceneNode[]): Promise<void> {
  const request: ReviewRequest = {
    type: "review",
    kind: exportKind,
    candidates: await Promise.all(nodes.map(exportCandidate)),
  };
  figma.ui.postMessage(request);
}

function isSceneNode(node: BaseNode | null): node is SceneNode {
  return node !== null && !node.removed && node.type !== "PAGE" && node.type !== "DOCUMENT";
}

// Layers that can't be converted are still exported as SVG so the dialog can show what they are.
async function exportCandidate(node: SceneNode): Promise<ExportCandidate> {
  const candidate: ExportCandidate = { nodeId: node.id, name: node.name };
  if ("width" in node) Object.assign(candidate, { width: node.width, height: node.height });
  let exportError: string | undefined;
  try {
    candidate.source = await node.exportAsync({ format: "SVG" });
  } catch (error) {
    exportError = error instanceof Error ? error.message : String(error);
  }
  if (node.type === "COMPONENT_SET") {
    candidate.skipReason = "Component sets can't be exported as one drawable. Select the variants instead.";
  } else if (!isConvertible(node)) {
    candidate.skipReason = "Only frames, components, and instances can be exported.";
  } else if (exportError !== undefined) {
    candidate.skipReason = `Figma couldn't export this layer as SVG: ${exportError}`;
  }
  return candidate;
}

// Zooms without changing the selection, so the user can fix a layer and refresh the dialog.
async function focus(nodeId: string): Promise<void> {
  const node = await figma.getNodeByIdAsync(nodeId);
  if (node && "absoluteBoundingBox" in node) figma.viewport.scrollAndZoomIntoView([node]);
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

function settle(id: string, result: ConvertResult): void {
  const request = typeof id === "string" ? pending.get(id) : undefined;
  if (!request) return;
  pending.delete(id);
  clearTimeout(request.timer);
  request.resolve(result);
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
  const shown = blockingDiagnostics(analysis?.diagnostics ?? []);
  return [
    {
      title: "Can't convert to Vector Drawable",
      language: "PLAINTEXT",
      code: shown.length > 0 ? formatDiagnostics(shown) : wrap(capitalize(message), ""),
    },
  ];
}

function warningResult(analysis: Analysis): CodegenResult[] {
  const warnings = warningDiagnostics(analysis.diagnostics);
  if (warnings.length === 0) return [];
  return [{ title: "Warnings", language: "PLAINTEXT", code: formatDiagnostics(warnings) }];
}

function formatDiagnostics(diagnostics: Diagnostic[]): string {
  return diagnostics.map(formatDiagnostic).join("\n\n");
}

// Source locations are omitted: they point into Figma's SVG export, which the user never sees.
function formatDiagnostic(diagnostic: Diagnostic): string {
  const summary = wrap(`• ${capitalize(diagnostic.message)} (${diagnostic.code})`, "  ");
  return diagnostic.suggestion ? `${summary}\n${wrap(`→ ${diagnostic.suggestion}`, "  ", "  ")}` : summary;
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
