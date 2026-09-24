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
      'export { matchesFilter, reviewCandidates } from "./src/export-review.ts";',
      'export { resourceName, uniqueResourceNames } from "./src/resource-names.ts";',
      'export { createDrawableZip, crc32 } from "./src/zip.ts";',
    ].join("\n"),
    resolveDir: new URL("..", import.meta.url).pathname,
    loader: "ts",
  },
  bundle: true,
  format: "esm",
  write: false,
});
const { matchesFilter, reviewCandidates, resourceName, uniqueResourceNames, createDrawableZip, crc32 } = await import(
  `data:text/javascript;base64,${Buffer.from(bundled.outputFiles[0].text).toString("base64")}`
);

const wasm = await import("../vendor/vdtoolkit-wasm/vdtoolkit_wasm.js");
await wasm.default({
  module_or_path: await readFile(new URL("../vendor/vdtoolkit-wasm/vdtoolkit_wasm_bg.wasm", import.meta.url)),
});
const convert = (source) => wasm.convertSvg(source, true);
const svg = (body) =>
  new TextEncoder().encode(`<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">${body}</svg>`);

function runExportCommand(selection, command = "export-vector-drawables") {
  const calls = { notify: [], showUI: [], posted: [], closed: false, zoomed: [] };
  const figma = {
    mode: "default",
    command,
    currentPage: { selection },
    showUI: (_html, options) => calls.showUI.push(options),
    notify: (message) => calls.notify.push(message),
    closePlugin: (message) => {
      calls.closed = true;
      if (message !== undefined) calls.notify.push(message);
    },
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

test("notification command selects notification conversion and labels its UI", async () => {
  const selection = [
    {
      id: "1",
      name: "Bell",
      type: "FRAME",
      width: 24,
      height: 24,
      exportAsync: async () => new Uint8Array([1]),
    },
  ];
  const { figma, calls } = runExportCommand(selection, "export-notification-icons");
  await settle();

  assert.equal(calls.showUI[0].title, "Export Notification Icons");
  assert.equal(calls.posted[0].kind, "notification");
  figma.ui.onmessage({ type: "exported", count: 2 });
  assert.equal(calls.notify.at(-1), "Exported 2 notification icons");
  assert.equal(calls.closed, true, "the dialog closes once the zip is saved");
});

test("export command sends every selected layer to the dialog, explaining skipped ones", async () => {
  const selection = [
    {
      id: "1",
      name: "Arrow",
      type: "FRAME",
      width: 24,
      height: 24,
      absoluteBoundingBox: {},
      exportAsync: async () => new Uint8Array([1]),
    },
    { id: "2", name: "Box", type: "RECTANGLE", absoluteBoundingBox: {}, exportAsync: async () => new Uint8Array([2]) },
    { id: "3", name: "Buttons", type: "COMPONENT_SET", absoluteBoundingBox: {}, exportAsync: async () => new Uint8Array([3]) },
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
    // Skipped layers keep their SVG so the dialog can show a thumbnail.
    Array.from(review.candidates, (candidate) => [
      candidate.nodeId,
      candidate.source?.[0],
      candidate.skipReason,
    ]),
    [
      ["1", 1, undefined],
      ["2", 2, "Only frames, components, and instances can be exported."],
      ["3", 3, "Component sets can't be exported as one drawable. Select the variants instead."],
      ["4", undefined, "Figma couldn't export this layer as SVG: export failed"],
    ],
  );

  assert.deepEqual([review.candidates[0].width, review.candidates[0].height], [24, 24]);

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
      {
        nodeId: "3",
        name: "Group",
        source: svg('<rect width="24" height="24"/>'),
        skipReason: "Only frames, components, and instances can be exported.",
      },
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
  assert.deepEqual([rows[0].exportWidth, rows[0].exportHeight], [24, 24]);
  assert.equal(rows[0].resized, false);
  assert.ok(rows.every((row) => row.preview instanceof Uint8Array), "every exported layer keeps its SVG preview");
  assert.ok(rows[1].issues.length > 0);
  const codes = rows[1].issues.map((issue) => issue.code);
  assert.ok(codes.every((code) => /^VDT\d{3}$/.test(code)));
  assert.equal(new Set(codes).size, codes.length, "each issue code is listed once");
  assert.deepEqual(rows[2].issues, [{ message: "Only frames, components, and instances can be exported." }]);
});

test("review exports large frames capped at 200dp and says so", () => {
  const hero = new TextEncoder().encode(
    '<svg xmlns="http://www.w3.org/2000/svg" width="480" height="320" viewBox="0 0 480 320"><rect width="480" height="320"/></svg>',
  );
  const [row] = reviewCandidates(
    [{ nodeId: "1", name: "Hero", source: hero, width: 480, height: 320 }],
    (source) => wasm.convertSvg(source, true, 200),
  );
  assert.equal(row.status, "ready");
  assert.deepEqual([row.width, row.height, row.exportWidth, row.exportHeight], [480, 320, 200, 133]);
  assert.match(row.xml, /android:width="200dp"/);
  assert.match(row.xml, /android:height="133dp"/);
  assert.equal(row.resized, true);
  assert.deepEqual(row.warnings, []);
});

test("notification review exports a white 24dp icon with icon diagnostics", () => {
  const source = svg('<rect width="24" height="24" fill="#123456"/>');
  const [row] = reviewCandidates(
    [{ nodeId: "1", name: "Status", source, width: 48, height: 48 }],
    (input) => wasm.convertNotificationSvg(input, true),
    "notification",
  );

  assert.equal(row.status, "ready");
  assert.deepEqual([row.exportWidth, row.exportHeight, row.resized], [24, 24, false]);
  assert.match(row.xml, /android:fillColor="#FFFFFF"/);
  assert.ok(row.warnings.some((warning) => warning.code === "VDT017"));
});

test("wide-gamut colors are resolved without saying so in the export dialog", () => {
  // Figma writes the sRGB fallback first and the wide-gamut color after it.
  // The stroke has to survive, but a notification icon flattens every color
  // to white, so saying anything about color would only be noise.
  const folder = new TextEncoder().encode(
    '<svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">' +
      '<path d="M3.25 3.25H16.75V16.75H3.25Z" stroke="#6C707E" stroke-width="1.5"' +
      ' style="stroke:#6C707E;stroke:color(display-p3 0.4235 0.4392 0.4941);stroke-opacity:1;"/></svg>',
  );
  const [row] = reviewCandidates(
    [{ nodeId: "1", name: "Project", source: folder, width: 20, height: 20 }],
    (input) => wasm.convertNotificationSvg(input, true),
    "notification",
  );

  assert.equal(row.status, "ready");
  assert.match(row.xml, /android:strokeColor="#FFFFFF"/);
  assert.deepEqual(row.warnings, [], "no color note, and no empty-artwork warning");
});

test("size is never listed as a reason a large layer can't be exported", () => {
  const blurredHero = new TextEncoder().encode(
    '<svg xmlns="http://www.w3.org/2000/svg" width="480" height="320" viewBox="0 0 480 320">' +
      '<filter id="b"><feGaussianBlur stdDeviation="2"/></filter><rect width="480" height="320" filter="url(#b)"/></svg>',
  );
  const [row] = reviewCandidates(
    [{ nodeId: "1", name: "Hero", source: blurredHero, width: 480, height: 320 }],
    (source) => wasm.convertSvg(source, true, 200),
  );
  assert.equal(row.status, "blocked");
  const codes = row.issues.map((issue) => issue.code);
  assert.ok(codes.includes("VDT002"), "the real blocker is listed");
  assert.ok(!codes.includes("VDT016"), "the size warning is not");
});

test("filter matches layer and file names, ignoring case and surrounding spaces", () => {
  const ready = { status: "ready", name: "Theme = Light", fileName: "folder_light.xml" };
  const blocked = { status: "blocked", name: "Blurred badge" };
  assert.equal(matchesFilter(ready, ""), true);
  assert.equal(matchesFilter(ready, "  theme = LIGHT "), true);
  assert.equal(matchesFilter(ready, "folder"), true);
  assert.equal(matchesFilter(ready, "dark"), false);
  assert.equal(matchesFilter(blocked, "BADGE"), true);
  assert.equal(matchesFilter(blocked, ".xml"), false);
});

test("layer names become valid Android resource names", () => {
  assert.equal(resourceName("Icons/Arrow Left"), "icons_arrow_left");
  assert.equal(resourceName("ChevronDown"), "chevron_down");
  assert.equal(resourceName("24px Café"), "ic_24px_cafe");
  assert.equal(resourceName("🙂"), "vector");
  assert.equal(resourceName("C++"), "c_plus_plus");
  assert.equal(resourceName("C#"), "c_sharp");
  assert.equal(resourceName("R&D @ 100%"), "r_and_d_at_100_percent");
  assert.deepEqual(uniqueResourceNames(["Home", "home", "HOME"]), ["home", "home_2", "home_3"]);
});

test("ZIP archive has one stored entry per drawable", () => {
  assert.equal(crc32(new TextEncoder().encode("123456789")), 0xcbf43926);
  const data = new TextEncoder().encode("<vector/>");
  const zip = createDrawableZip([
    { path: "a.xml", data },
    { path: "b.xml", data },
  ]);
  const view = new DataView(zip.buffer);
  assert.equal(view.getUint32(0, true), 0x04034b50);
  const firstNameLength = view.getUint16(26, true);
  assert.equal(new TextDecoder().decode(zip.subarray(30, 30 + firstNameLength)), "drawable-anydpi/a.xml");
  const end = zip.length - 22;
  assert.equal(view.getUint32(end, true), 0x06054b50);
  assert.equal(view.getUint16(end + 10, true), 2);
  const centralOffset = view.getUint32(end + 16, true);
  assert.equal(view.getUint32(centralOffset, true), 0x02014b50);
});
