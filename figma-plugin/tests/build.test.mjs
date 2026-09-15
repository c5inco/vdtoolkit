import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url)));
const code = await readFile(new URL("../dist/code.js", import.meta.url), "utf8");
const icon = { diagnostics: [], metrics: { width: 24, height: 24, viewport_width: 24, viewport_height: 24 } };
const ui = await readFile(new URL("../dist/ui.html", import.meta.url), "utf8");

test("manifest declares a network-free codegen plugin with a Design Mode export command", () => {
  assert.deepEqual(manifest.editorType, ["figma", "dev"]);
  assert.deepEqual(
    manifest.menu.map((item) => item.command),
    ["export-vector-drawables", "export-notification-icons"],
  );
  assert.deepEqual(manifest.capabilities, ["codegen"]);
  assert.deepEqual(manifest.networkAccess.allowedDomains, ["none"]);
  assert.equal(manifest.main, "dist/code.js");
  assert.equal(manifest.ui, "dist/ui.html");
  assert.deepEqual(
    manifest.codegenLanguages.map((language) => language.value),
    ["android-vectordrawable"],
  );
});

test("plugin bundles are self-contained and preserve the node-type guard", () => {
  for (const type of ["FRAME", "COMPONENT", "INSTANCE"]) {
    assert.match(code, new RegExp(`\\.type\\s*===\\s*"${type}"`));
  }
  assert.match(code, /showUI\([^)]*,\s*\{\s*visible:\s*false\s*\}/);
  assert.match(code, /postMessage/);
  assert.match(ui, /^<!doctype html>/);
  // The export dialog draws into document.body as soon as the script runs.
  assert.match(ui, /<body><script>/);
  assert.match(ui, /AGFzbQE/); // base64-encoded WebAssembly magic bytes
  assert.match(ui, /White notification icon preview/);
  assert.match(ui, /brightness\(0\) invert\(1\)/);
  assert.match(ui, /dark background is for preview only.*Exported icons stay transparent/i);
  assert.doesNotMatch(ui, /https?:\/\//);
  assert.doesNotMatch(ui, /<script[^>]+src=/);
});

test("sandbox ignores unsupported nodes and correlates concurrent iframe responses", async () => {
  let generate;
  const posted = [];
  const figma = {
    mode: "codegen",
    showUI: (_html, options) => assert.equal(options.visible, false),
    ui: {
      onmessage: undefined,
      postMessage: (message) => posted.push(message),
    },
    codegen: {
      on: (event, handler) => {
        assert.equal(event, "generate");
        generate = handler;
      },
    },
  };
  vm.runInNewContext(code, { figma, __html__: "", setTimeout, clearTimeout });

  assert.equal((await generate({ node: { type: "RECTANGLE" } })).length, 0);
  assert.equal((await generate({ node: { type: "COMPONENT_SET" } })).length, 0);

  const first = generate({
    node: { type: "FRAME", exportAsync: async () => new Uint8Array([1]) },
  });
  const second = generate({
    node: { type: "COMPONENT", exportAsync: async () => new Uint8Array([2]) },
  });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(posted.length, 2);
  assert.equal("maxSizeDp" in posted[0], false);
  assert.notEqual(posted[0].id, posted[1].id);

  figma.ui.onmessage({
    type: "converted",
    id: posted[1].id,
    result: { ok: true, analysis: icon, xml: "<vector second/>" },
  });
  figma.ui.onmessage({
    type: "converted",
    id: posted[0].id,
    result: { ok: true, analysis: icon, xml: "<vector first/>" },
  });

  assert.equal((await first)[0].code, "<vector first/>");
  assert.equal((await second)[0].code, "<vector second/>");

  const unsupported = generate({
    node: { type: "INSTANCE", exportAsync: async () => new Uint8Array([3]) },
  });
  await new Promise((resolve) => setImmediate(resolve));
  figma.ui.onmessage({
    type: "converted",
    id: posted[2].id,
    result: {
      ok: false,
      error: {
        kind: "unsupported",
        message: "unsupported",
        analysis: {
          diagnostics: [
            { code: "SVGVD011", severity: "info", message: "safe SVG normalization is required" },
            { code: "SVGVD003", severity: "error", message: "gradient paint" },
            {
              code: "SVGVD007",
              severity: "error",
              message: "embedded raster or SVG images are unsupported",
              location: { element: "image", line: 4, column: 2 },
              suggestion: "Replace the image with vector path geometry.",
            },
            { code: "SVGVD003", severity: "error", message: "gradient paint" },
          ],
        },
      },
    },
  });
  const [diagnostics] = await unsupported;
  assert.equal(diagnostics.title, "Can't convert to Vector Drawable");
  assert.equal(
    diagnostics.code,
    [
      "• Gradient paint (SVGVD003)",
      "",
      "• Embedded raster or SVG images are unsupported",
      "  (SVGVD007)",
      "  → Replace the image with vector path geometry.",
    ].join("\n"),
  );
  assert.ok(diagnostics.code.split("\n").every((line) => line.length <= 52));
});

test("large drawables keep their size and surface warnings", async () => {
  let generate;
  const posted = [];
  const figma = {
    mode: "codegen",
    showUI() {},
    ui: { onmessage: undefined, postMessage: (message) => posted.push(message) },
    codegen: { on: (_event, handler) => (generate = handler) },
  };
  vm.runInNewContext(code, { figma, __html__: "", setTimeout, clearTimeout });

  const pending = generate({
    node: { type: "FRAME", exportAsync: async () => new Uint8Array([1]) },
  });
  await new Promise((resolve) => setImmediate(resolve));
  figma.ui.onmessage({
    type: "converted",
    id: posted[0].id,
    result: {
      ok: true,
      xml: "<vector/>",
      analysis: {
        metrics: { width: 480, height: 320, viewport_width: 480, viewport_height: 320 },
        diagnostics: [
          { code: "SVGVD011", severity: "info", message: "safe SVG normalization is required" },
          { code: "SVGVD016", severity: "warning", message: "480×320dp is larger than 200×200dp" },
        ],
      },
    },
  });
  const [drawable, warnings] = await pending;
  assert.equal(drawable.title, "Android Vector Drawable");
  assert.equal(drawable.code, "<vector/>");
  assert.equal(warnings.title, "Warnings");
  assert.equal(warnings.code, "• 480×320dp is larger than 200×200dp (SVGVD016)");
});
