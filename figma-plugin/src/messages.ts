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
