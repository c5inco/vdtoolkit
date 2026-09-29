declare module "*.wasm" {
  const bytes: Uint8Array;
  export default bytes;
}

// A fresh copy of wasm-bindgen's glue per call; see build.mjs.
declare module "*?factory" {
  const create: () => typeof import("../vendor/vdtoolkit-wasm/vdtoolkit_wasm");
  export default create;
}
