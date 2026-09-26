import createBindings from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm.js?factory";
import wasmBytes from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm_bg.wasm";
import { renderExportDialog, renderPreparing } from "./export-dialog";
import { reviewCandidates } from "./export-review";
import type { ConvertRequest, ConvertResponse, ReviewRequest, SandboxMessage } from "./messages";
import { converter, wasmSession, type WithWasm } from "./wasm-session";

declare const parent: Window;

// One compilation is shared by every request received by this iframe, including codegen
// requests that arrive concurrently. Instances are made from it synchronously, so one that
// trapped is replaced before the next layer of a batch. If compiling fails, each request
// reports the error. build.mjs imports the .wasm file as its bytes; TypeScript instead
// finds wasm-bindgen's declaration of the module's exports beside it.
const wasmReady: Promise<WithWasm> = WebAssembly.compile(wasmBytes as unknown as Uint8Array<ArrayBuffer>).then(
  (module) =>
    wasmSession(() => {
      const bindings = createBindings();
      bindings.initSync({ module });
      return bindings;
    }),
  (error: unknown) =>
    wasmSession(() => {
      throw error;
    }),
);

window.onmessage = async (event: MessageEvent<{ pluginMessage?: ConvertRequest | ReviewRequest }>) => {
  const request = event.data.pluginMessage;
  if (request?.type === "convert" && typeof request.id === "string") {
    // The Code panel is read by people, so it keeps readable XML; exported files are compact.
    const convert = converter(await wasmReady, "drawable", true, false);
    const response: ConvertResponse = { type: "converted", id: request.id, result: convert(request.source) };
    post(response);
  } else if (request?.type === "review" && Array.isArray(request.candidates)) {
    const withWasm = await wasmReady;
    const convert = converter(withWasm, request.kind);
    const resourceName = (name: string) => withWasm((wasm) => wasm.resourceName(name));
    renderExportDialog(reviewCandidates(request.candidates, convert, resourceName, request.kind), request.kind, post);
  }
};

// The same iframe serves the hidden Dev Mode converter and the visible Design Mode
// export dialog; invisible in codegen, this placeholder only shows while exporting.
// It is drawn after the listener is registered so a DOM failure can't break conversion.
if (document.body) renderPreparing();
else document.addEventListener("DOMContentLoaded", renderPreparing, { once: true });

function post(message: SandboxMessage): void {
  parent.postMessage({ pluginMessage: message }, "*");
}
