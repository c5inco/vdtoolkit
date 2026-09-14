import type { ConvertResult } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";

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

// One selected layer offered for batch export. `source` is absent when the layer
// isn't a frame, component, or instance, or when Figma's SVG export failed.
export interface ExportCandidate {
  nodeId: string;
  name: string;
  source?: Uint8Array;
  skipReason?: string;
}

export interface ReviewRequest {
  type: "review";
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
