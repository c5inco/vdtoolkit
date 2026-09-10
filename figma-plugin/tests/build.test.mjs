import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url)));
const code = await readFile(new URL("../dist/code.js", import.meta.url), "utf8");
const ui = await readFile(new URL("../dist/ui.html", import.meta.url), "utf8");

test("manifest declares a network-free Dev Mode codegen plugin", () => {
  assert.deepEqual(manifest.editorType, ["dev"]);
  assert.deepEqual(manifest.capabilities, ["codegen"]);
  assert.deepEqual(manifest.networkAccess.allowedDomains, ["none"]);
  assert.equal(manifest.main, "dist/code.js");
  assert.equal(manifest.ui, "dist/ui.html");
  assert.equal(manifest.codegenLanguages[0].value, "android-vectordrawable");
});

test("plugin bundles are self-contained and preserve the FRAME guard", () => {
  assert.match(code, /\.type\s*!==\s*"FRAME"/);
  assert.match(code, /showUI\([^)]*,\s*\{\s*visible:\s*false\s*\}/);
  assert.match(code, /postMessage/);
  assert.match(ui, /^<!doctype html>/);
  assert.match(ui, /AGFzbQE/); // base64-encoded WebAssembly magic bytes
  assert.doesNotMatch(ui, /https?:\/\//);
  assert.doesNotMatch(ui, /<script[^>]+src=/);
});

test("sandbox ignores non-frames and correlates concurrent iframe responses", async () => {
  let generate;
  const posted = [];
  const figma = {
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

  const first = generate({
    node: { type: "FRAME", exportAsync: async () => new Uint8Array([1]) },
  });
  const second = generate({
    node: { type: "FRAME", exportAsync: async () => new Uint8Array([2]) },
  });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(posted.length, 2);
  assert.notEqual(posted[0].id, posted[1].id);

  figma.ui.onmessage({
    type: "converted",
    id: posted[1].id,
    result: { ok: true, analysis: {}, xml: "<vector second/>" },
  });
  figma.ui.onmessage({
    type: "converted",
    id: posted[0].id,
    result: { ok: true, analysis: {}, xml: "<vector first/>" },
  });

  assert.equal((await first)[0].code, "<vector first/>");
  assert.equal((await second)[0].code, "<vector second/>");

  const unsupported = generate({
    node: { type: "FRAME", exportAsync: async () => new Uint8Array([3]) },
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
          diagnostics: [{ code: "SVGVD003", severity: "error", message: "gradient paint" }],
        },
      },
    },
  });
  assert.match((await unsupported)[0].code, /\[SVGVD003\] gradient paint/);
});
