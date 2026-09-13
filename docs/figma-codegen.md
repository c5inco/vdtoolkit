# Figma Dev Mode codegen proof of concept

The development plugin converts exactly one selected `FRAME`, `COMPONENT`, or
`INSTANCE` into optimized Android VectorDrawable XML in Figma's native Code
panel. All other node types, including component sets, return no code. It has no visible UI and makes no network requests.

Figma's plugin sandbox cannot instantiate WebAssembly. The sandbox exports the
selected node as SVG and sends its bytes, with a correlation ID, to a hidden iframe.
The iframe initializes one locally embedded `vdtoolkit` WebAssembly module and
reuses it for concurrent requests. Conversion failures are returned as
diagnostic code blocks instead of plugin crashes.

Android warns when a vector icon's `android:width` or `android:height` exceeds
200dp, because drawing it gets slow. The plugin keeps Figma's dimensions and
shows the `SVGVD016` warning in a separate **Warnings** block under the XML.
(A `select` codegen preference was tried for an opt-in size cap, but Figma's
Dev Mode shows the dropdown without delivering the chosen value to the plugin:
`figma.codegen.preferences.customSettings` stays undefined.)

## Build and test

Install Rust 1.85 or newer, `wasm-pack`, Node.js, and npm. Android tooling is
not needed.

```sh
rustup target add wasm32-unknown-unknown
cd figma-plugin
npm ci
npm run build
npm run typecheck
npm test
```

`npm run build` compiles the browser-targeted WASM package and emits the
self-contained plugin files in `figma-plugin/dist`. The generated
`dist/ui.html` embeds both the JavaScript glue and WASM bytes; it does not load
a CDN or other remote resource.

To test the generated WebAssembly module itself:

```sh
wasm-pack build bindings/wasm --target web --release --out-dir pkg
node --test bindings/wasm/tests/node.mjs
```

## Obtain a development plugin ID

Figma assigns plugin IDs, so the repository intentionally contains the
placeholder `REPLACE_WITH_YOUR_FIGMA_PLUGIN_ID`.

1. In the Figma desktop app, open **Plugins → Development → New plugin** and
   create a plugin owned by your account or development organization.
2. Copy the generated manifest's `id` value into `figma-plugin/manifest.json`,
   replacing the placeholder. Keep the rest of this repository's manifest.
3. In **Plugins → Development → Import plugin from manifest**, select
   `figma-plugin/manifest.json`.

The ID is local developer configuration for this proof of concept and should
not be committed unless the repository adopts a shared registered plugin.

## Manual test

1. Run `npm run build` after each source change, then reload the development
   plugin in Figma.
2. Open a Figma Design file, switch to Dev Mode, and select exactly one frame,
   component, or instance.
3. In the Code panel, choose **Android Vector Drawable**. A compatible node
   produces a code block titled **Android Vector Drawable** containing XML.
4. Select a rectangle, group, component set, section, or any other
   unsupported node. The plugin should contribute no code block.
5. To exercise diagnostics, select a frame or component whose SVG export contains an
   unsupported effect such as a blur/filter. The Code panel should show a
   **Can't convert to Vector Drawable** block listing each blocking issue with its stable `SVGVDnnn` code.
6. Select a frame larger than 200×200. The XML keeps Figma's size and a
   **Warnings** block shows `SVGVD016`.

V0 intentionally does not support multi-selection, other node types,
preferences, Figma for VS Code, or publication. Figma controls SVG export, so
the generated drawable reflects Figma's exported representation rather than
the original editable object model.
