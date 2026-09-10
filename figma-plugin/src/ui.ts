import init, { convertSvg } from "../vendor/svg2vd-wasm/svg2vd_wasm.js";
import wasmBytes from "../vendor/svg2vd-wasm/svg2vd_wasm_bg.wasm";
import type { ConvertResult } from "../vendor/svg2vd-wasm/svg2vd_wasm";
import type { ConvertRequest, ConvertResponse } from "./messages";

declare const parent: Window;

// One initialization promise is shared by every codegen request received by
// this hidden iframe, including requests that arrive concurrently.
const wasmReady = init({ module_or_path: wasmBytes });

window.onmessage = async (event: MessageEvent<{ pluginMessage?: ConvertRequest }>) => {
  const request = event.data.pluginMessage;
  if (!request || request.type !== "convert" || typeof request.id !== "string") return;

  let result: ConvertResult;
  try {
    await wasmReady;
    result = convertSvg(new Uint8Array(request.source), true);
  } catch (error) {
    result = {
      ok: false,
      error: {
        kind: "svg",
        message: error instanceof Error ? error.message : String(error),
      },
    };
  }

  const response: ConvertResponse = { type: "converted", id: request.id, result };
  parent.postMessage({ pluginMessage: response }, "*");
};
