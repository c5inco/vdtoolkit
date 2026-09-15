import init, { convertSvg } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm.js";
import wasmBytes from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm_bg.wasm";
import type { ConvertResult } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";
import { renderExportDialog, renderPreparing } from "./export-dialog";
import { MAX_EXPORT_DP, reviewCandidates } from "./export-review";
import type { ConvertRequest, ConvertResponse, ReviewRequest, SandboxMessage } from "./messages";

declare const parent: Window;

// One initialization promise is shared by every request received by this
// iframe, including codegen requests that arrive concurrently.
// It settles with the initialization error, if any, so each request can report it.
const wasmReady: Promise<unknown> = init({ module_or_path: wasmBytes }).then(
  () => undefined,
  (error: unknown) => error,
);

window.onmessage = async (event: MessageEvent<{ pluginMessage?: ConvertRequest | ReviewRequest }>) => {
  const request = event.data.pluginMessage;
  if (request?.type === "convert" && typeof request.id === "string") {
    const convert = converter(await wasmReady);
    const response: ConvertResponse = { type: "converted", id: request.id, result: convert(request.source) };
    post(response);
  } else if (request?.type === "review" && Array.isArray(request.candidates)) {
    const convert = converter(await wasmReady, MAX_EXPORT_DP);
    renderExportDialog(reviewCandidates(request.candidates, convert), post);
  }
};

// The same iframe serves the hidden Dev Mode converter and the visible Design Mode
// export dialog; invisible in codegen, this placeholder only shows while exporting.
// It is drawn after the listener is registered so a DOM failure can't break conversion.
if (document.body) renderPreparing();
else document.addEventListener("DOMContentLoaded", renderPreparing, { once: true });

function converter(initError: unknown, maxSizeDp?: number): (source: Uint8Array) => ConvertResult {
  return (source) => {
    try {
      if (initError !== undefined) throw initError;
      return convertSvg(new Uint8Array(source), true, maxSizeDp);
    } catch (error) {
      return {
        ok: false,
        error: {
          kind: "svg",
          message: error instanceof Error ? error.message : String(error),
        },
      };
    }
  };
}

function post(message: SandboxMessage): void {
  parent.postMessage({ pluginMessage: message }, "*");
}
