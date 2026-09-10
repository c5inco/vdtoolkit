import type { ConvertResult } from "../vendor/svg2vd-wasm/svg2vd_wasm";

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
