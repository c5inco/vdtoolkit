import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { build } from "esbuild";

const code = await readFile(new URL("../dist/code.js", import.meta.url), "utf8");

// The review, naming, and ZIP helpers are browser-free, so bundle them straight from source.
const bundled = await build({
  stdin: {
    contents: [
      'export { reviewCandidates } from "./src/export-review.ts";',
      'export { resourceName, uniqueResourceNames } from "./src/resource-names.ts";',
      'export { createZip, crc32 } from "./src/zip.ts";',
    ].join("\n"),
    resolveDir: new URL("..", import.meta.url).pathname,
    loader: "ts",
  },
  bundle: true,
  format: "esm",
  write: false,
});
const { reviewCandidates, resourceName, uniqueResourceNames, createZip, crc32 } = await import(
  `data:text/javascript;base64,${Buffer.from(bundled.outputFiles[0].text).toString("base64")}`
);

const wasm = await import("../vendor/vdtoolkit-wasm/vdtoolkit_wasm.js");
await wasm.default({
  module_or_path: await readFile(new URL("../vendor/vdtoolkit-wasm/vdtoolkit_wasm_bg.wasm", import.meta.url)),
});
const convert = (source) => wasm.convertSvg(source, true);
const svg = (body) =>
  new TextEncoder().encode(`<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">${body}</svg>`);

function runExportCommand(selection) {
  const calls = { notify: [], showUI: [], posted: [], closed: false, zoomed: [] };
  const figma = {
    mode: "default",
    command: "export-vector-drawables",
    currentPage: { selection },
    showUI: (_html, options) => calls.showUI.push(options),
    notify: (message) => calls.notify.push(message),
    closePlugin: () => (calls.closed = true),
    getNodeByIdAsync: async (id) => selection.find((node) => node.id === id) ?? null,
    viewport: { scrollAndZoomIntoView: (nodes) => calls.zoomed.push(...nodes.map((node) => node.id)) },
    ui: { onmessage: undefined, postMessage: (message) => calls.posted.push(message) },
  };
  vm.runInNewContext(code, { figma, __html__: "", setTimeout, clearTimeout });
  return { figma, calls };
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

test("export command without a selection explains what to select", () => {
  const { calls } = runExportCommand([]);
  assert.equal(calls.showUI.length, 0);
  assert.match(calls.notify[0], /Select one or more frames/);
  assert.equal(calls.closed, true);
});

test("export command sends every selected layer to the dialog, explaining skipped ones", async () => {
  const selection = [
    { id: "1", name: "Arrow", type: "FRAME", absoluteBoundingBox: {}, exportAsync: async () => new Uint8Array([1]) },
    { id: "2", name: "Box", type: "RECTANGLE", absoluteBoundingBox: {} },
    { id: "3", name: "Buttons", type: "COMPONENT_SET", absoluteBoundingBox: {} },
    {
      id: "4",
      name: "Broken",
      type: "INSTANCE",
      absoluteBoundingBox: {},
      // A string, because an Error from this realm fails the sandbox's `instanceof Error`.
      exportAsync: async () => {
        throw "export failed";
      },
    },
  ];
  const { figma, calls } = runExportCommand(selection);
  await settle();

  assert.notEqual(calls.showUI[0].visible, false);
  const [review] = calls.posted;
  assert.equal(review.type, "review");
  assert.deepEqual(
    Array.from(review.candidates, (candidate) => [candidate.nodeId, candidate.source ? "source" : candidate.skipReason]),
    [
      ["1", "source"],
      ["2", "Only frames, components, and instances can be exported."],
      ["3", "Component sets can't be exported as one drawable. Select the variants instead."],
      ["4", "Figma couldn't export this layer as SVG: export failed"],
    ],
  );

  await figma.ui.onmessage({ type: "focus", nodeId: "4" });
  assert.deepEqual(calls.zoomed, ["4"]);
  figma.ui.onmessage({ type: "exported", count: 2 });
  assert.equal(calls.notify.at(-1), "Exported 2 Vector Drawables");
  figma.ui.onmessage({ type: "close" });
  assert.equal(calls.closed, true);
});

test("refresh re-checks the original layers, not the current selection", async () => {
  let exportCount = 0;
  const frame = (id, name) => ({
    id,
    name,
    type: "FRAME",
    absoluteBoundingBox: {},
    exportAsync: async () => new Uint8Array([++exportCount]),
  });
  const selection = [frame("1", "Arrow"), frame("2", "Home")];
  const { figma, calls } = runExportCommand(selection);
  await settle();

  // The user selects one layer to fix it, renames it, and deletes the other.
  figma.currentPage.selection = [selection[0]];
  selection[0].name = "Arrow Fixed";
  selection.splice(1, 1);
  await figma.ui.onmessage({ type: "refresh" });

  const refreshed = calls.posted.at(-1);
  assert.equal(refreshed.type, "review");
  assert.deepEqual(
    Array.from(refreshed.candidates, (candidate) => [candidate.nodeId, candidate.name, candidate.source[0]]),
    [["1", "Arrow Fixed", 3]],
  );
});

test("review marks convertible layers ready and lists blockers for the rest", () => {
  const rows = reviewCandidates(
    [
      { nodeId: "1", name: "Arrow Left", source: svg('<path d="M4 12h16" stroke="#000" stroke-width="2"/>') },
      {
        nodeId: "2",
        name: "Blurred",
        source: svg('<filter id="b"><feGaussianBlur stdDeviation="2"/></filter><rect width="24" height="24" filter="url(#b)"/>'),
      },
      { nodeId: "3", name: "Group", skipReason: "Only frames, components, and instances can be exported." },
      { nodeId: "4", name: "arrow-left", source: svg('<rect width="24" height="24"/>') },
    ],
    convert,
  );

  assert.deepEqual(
    rows.map((row) => [row.name, row.status, row.fileName]),
    [
      ["Arrow Left", "ready", "arrow_left.xml"],
      ["Blurred", "blocked", undefined],
      ["Group", "blocked", undefined],
      ["arrow-left", "ready", "arrow_left_2.xml"],
    ],
  );
  assert.match(rows[0].xml, /<vector xmlns:android=/);
  assert.ok(rows[1].issues.length > 0);
  const codes = rows[1].issues.map((issue) => issue.code);
  assert.ok(codes.every((code) => /^SVGVD\d{3}$/.test(code)));
  assert.equal(new Set(codes).size, codes.length, "each issue code is listed once");
  assert.deepEqual(rows[2].issues, [{ message: "Only frames, components, and instances can be exported." }]);
});

test("layer names become valid Android resource names", () => {
  assert.equal(resourceName("Icons/Arrow Left"), "icons_arrow_left");
  assert.equal(resourceName("ChevronDown"), "chevron_down");
  assert.equal(resourceName("24px Café"), "ic_24px_cafe");
  assert.equal(resourceName("🙂"), "vector");
  assert.deepEqual(uniqueResourceNames(["Home", "home", "HOME"]), ["home", "home_2", "home_3"]);
});

test("ZIP archive has one stored entry per drawable", () => {
  assert.equal(crc32(new TextEncoder().encode("123456789")), 0xcbf43926);
  const data = new TextEncoder().encode("<vector/>");
  const zip = createZip([
    { path: "a.xml", data },
    { path: "b.xml", data },
  ]);
  const view = new DataView(zip.buffer);
  assert.equal(view.getUint32(0, true), 0x04034b50);
  const end = zip.length - 22;
  assert.equal(view.getUint32(end, true), 0x06054b50);
  assert.equal(view.getUint16(end + 10, true), 2);
  const centralOffset = view.getUint32(end + 16, true);
  assert.equal(view.getUint32(centralOffset, true), 0x02014b50);
});
