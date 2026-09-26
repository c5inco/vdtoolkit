import type { ConvertResult, VdtoolkitError } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";

export type ExportKind = "drawable" | "notification";

// vdtoolkit's result, or the iframe's own failure: the WebAssembly module didn't load or
// crashed partway through a conversion.
export type ConversionResult = ConvertResult | { ok: false; error: InternalError };

export interface InternalError extends Omit<VdtoolkitError, "kind"> {
  kind: "internal";
}

export interface ConvertRequest {
  type: "convert";
  id: string;
  source: Uint8Array;
}

export interface ConvertResponse {
  type: "converted";
  id: string;
  result: ConversionResult;
}

// One selected layer offered for batch export. `source` is Figma's SVG export, shown as
// the thumbnail and converted unless `skipReason` says the layer can't be exported.
export interface ExportCandidate {
  nodeId: string;
  name: string;
  // The layer's size in Figma, which its SVG export keeps as the drawable's dp size.
  width?: number;
  height?: number;
  source?: Uint8Array;
  skipReason?: string;
}

export interface ReviewRequest {
  type: "review";
  kind: ExportKind;
  candidates: ExportCandidate[];
}

export interface FocusRequest {
  type: "focus";
  nodeId: string;
}

export interface ExportedNotice {
  type: "exported";
  count: number;
}

export interface CloseRequest {
  type: "close";
}

// Re-checks the layers the dialog opened with, after the user has edited them.
export interface RefreshRequest {
  type: "refresh";
}

export type SandboxMessage = ConvertResponse | FocusRequest | ExportedNotice | CloseRequest | RefreshRequest;
