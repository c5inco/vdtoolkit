import { MAX_EXPORT_DP } from "./export-review";
import type { ConversionResult, ExportKind } from "./messages";

export type Bindings = typeof import("../vendor/vdtoolkit-wasm/vdtoolkit_wasm");
export type WithWasm = <T>(call: (bindings: Bindings) => T) => T;

// wasm32-unknown-unknown turns a Rust panic into a trap, which abandons the call wherever
// it was: the stack pointer, the allocator, and whatever state was being updated are left
// half-done. So an instance is dropped as soon as a call into it throws, and the next call
// gets a new one. `instantiate` is synchronous so that the rest of a batch never reaches
// the old instance.
export function wasmSession(instantiate: () => Bindings): WithWasm {
  let current: Bindings | undefined;
  return (call) => {
    const bindings = (current ??= instantiate());
    try {
      return call(bindings);
    } catch (error) {
      current = undefined;
      throw error;
    }
  };
}

export function converter(
  withWasm: WithWasm,
  kind: ExportKind = "drawable",
  pretty = false,
  capDrawable = true,
): (source: Uint8Array) => ConversionResult {
  return (source) => {
    try {
      return withWasm((wasm) =>
        kind === "notification"
          ? wasm.convertNotificationSvg(new Uint8Array(source), true, undefined, pretty, true)
          : wasm.convertSvg(new Uint8Array(source), true, capDrawable ? MAX_EXPORT_DP : undefined, pretty, true),
      );
    } catch (error) {
      // vdtoolkit returns every problem with an SVG as a result, so a throw means the
      // module failed to load or crashed: nothing a change to the layer would fix.
      return {
        ok: false,
        error: { kind: "internal", message: `internal error in vdtoolkit, not in the layer: ${String(error)}` },
      };
    }
  };
}
