import type { ConvertResult } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";

export type ExportKind = "drawable" | "notification";

export interface ConvertRequest {
  type: "convert";
  id: string;
  source: Uint8Array;
}

export interface ConvertResponse {
  type: "converted";
  id: string;
  result: ConvertResult;
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
