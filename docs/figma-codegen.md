# Figma Dev Mode codegen proof of concept

The development plugin converts exactly one selected `FRAME` into optimized
Android VectorDrawable XML in Figma's native Code panel. All other node types
return no code. It has no visible UI and makes no network requests.

Figma's plugin sandbox cannot instantiate WebAssembly. The sandbox exports the
frame as SVG and sends its bytes, with a correlation ID, to a hidden iframe.
The iframe initializes one locally embedded `svg2vd` WebAssembly module and
reuses it for concurrent requests. Conversion failures are returned as
diagnostic code blocks instead of plugin crashes.

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
2. Open a Figma Design file, switch to Dev Mode, and select exactly one frame.
3. In the Code panel, choose **Android VectorDrawable**. A compatible frame
   produces a code block titled **Android VectorDrawable** containing XML.
4. Select a rectangle, group, component, instance, section, or any other
   non-frame node. The plugin should contribute no code block.
5. To exercise diagnostics, select a frame whose SVG export contains an
   unsupported effect such as a gradient. The Code panel should show an
   **svg2vd diagnostics** block with stable `SVGVDnnn` codes where available.

V0 intentionally does not support multi-selection, non-frame nodes,
preferences, Figma for VS Code, or publication. Figma controls SVG export, so
the generated drawable reflects Figma's exported representation rather than
the original editable object model.
