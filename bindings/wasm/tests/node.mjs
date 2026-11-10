import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import init, { analyzeSvg, convertNotificationSvg, convertSvg, resourceName } from "../pkg/vdtoolkit_wasm.js";

const wasm = await readFile(new URL("../pkg/vdtoolkit_wasm_bg.wasm", import.meta.url));
await init({ module_or_path: wasm });

const bytes = (source) => new TextEncoder().encode(source);
const exact = bytes('<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><path d="M0 0H20V10Z" fill="#123456"/></svg>');
const normalized = bytes('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect x="1" y="2" width="3" height="4" fill="#123456"/></svg>');
const unsupported = bytes('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><text>hello</text></svg>');
const malformed = bytes("<svg><path></svg>");
const decimals = bytes('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M1.234567 2.345678L20.987654 21.876543" fill="#123456"/></svg>');
const large = bytes('<svg xmlns="http://www.w3.org/2000/svg" width="480" height="320"><path d="M0 0H480V320Z" fill="#123456"/></svg>');

test("the generated module returns exact and normalized analysis", () => {
  assert.equal(analyzeSvg(exact).analysis.compatibility, "exact");
  const result = convertSvg(normalized, false);
  assert.equal(result.ok, true);
  assert.equal(result.analysis.compatibility, "exact_with_normalization");
  assert.match(result.xml, /android:pathData="M1,2 L4,2 L4,6 L1,6 Z"/);
  const pretty = convertSvg(normalized, false, undefined, true);
  const readableBytes = Buffer.byteLength(pretty.xml, "utf8");
  assert.equal(result.analysis.metrics.estimated_xml_bytes, readableBytes);
  // The field order the CLI and binding wrote before the size became a method.
  assert.deepEqual(Object.keys(result.analysis.metrics), [
    "width",
    "height",
    "viewport_width",
    "viewport_height",
    "paths",
    "path_commands",
    "groups",
    "gradients",
    "clip_paths",
    "content_bounds",
    "estimated_xml_bytes",
  ]);
  assert.equal(pretty.analysis.metrics.estimated_xml_bytes, readableBytes);
  assert.equal(analyzeSvg(normalized).analysis.metrics.estimated_xml_bytes, readableBytes);
});

test("unsupported and malformed inputs are structured failures", () => {
  const unsupportedResult = convertSvg(unsupported, false);
  assert.equal(unsupportedResult.ok, false);
  assert.equal(unsupportedResult.error.kind, "unsupported");
  assert.equal(unsupportedResult.error.analysis.compatibility, "unsupported");
  assert.ok(unsupportedResult.error.analysis.diagnostics.some(({ code }) => code === "VDT006"));
  assert.equal(unsupportedResult.error.analysis.metrics.estimated_xml_bytes, 0);
  assert.equal(analyzeSvg(unsupported).analysis.metrics.estimated_xml_bytes, 0);

  const malformedResult = convertSvg(malformed, false);
  assert.equal(malformedResult.ok, false);
  assert.equal(malformedResult.error.kind, "xml");
  assert.equal("analysis" in malformedResult.error, false);
});

test("deep nesting is a structured failure rather than a trap", () => {
  // Depth counts the root and the leaf; nested <svg> uses the most stack.
  const nested = (open, close, depth) =>
    bytes(`<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">${open.repeat(depth - 2)}<path d="M1 1h2v2z"/>${close.repeat(depth - 2)}</svg>`);
  const svg = ['<svg x="0" y="0" width="24" height="24" viewBox="0 0 24 24">', "</svg>"];
  assert.equal(convertSvg(nested(...svg, 200), false).ok, true);
  assert.equal(analyzeSvg(nested(...svg, 200)).ok, true);
  assert.equal(convertNotificationSvg(nested(...svg, 200), false).ok, true);
  for (const deep of [nested("<g>", "</g>", 201), nested("<g>", "</g>", 20_000), nested(...svg, 20_000)]) {
    const result = convertSvg(deep, false);
    assert.equal(result.ok, false);
    assert.equal(result.error.kind, "invalid_input");
    assert.match(result.error.message, /nested more than 200/);
  }
});

test("optimization is effective and output is deterministic", () => {
  const plain = convertSvg(decimals, false);
  const optimized = convertSvg(decimals, true);
  assert.ok(optimized.xml.length < plain.xml.length);
  assert.match(optimized.xml, /android:pathData="M1\.235 2\.346/);
  // The estimate measures the readable XML, which `pretty` returns.
  const pretty = convertSvg(decimals, true, undefined, true);
  assert.equal(optimized.analysis.metrics.estimated_xml_bytes, pretty.xml.length);
  assert.ok(pretty.xml.includes("\n"));
  assert.ok(!optimized.xml.includes("\n"));
  assert.deepEqual(convertSvg(exact, false), convertSvg(exact, false));
});

test("large drawables warn and can be fit within a size cap", () => {
  const uncapped = convertSvg(large, false);
  assert.deepEqual(
    uncapped.analysis.diagnostics.map(({ code, severity }) => [code, severity]),
    [["VDT016", "warning"]],
  );
  assert.match(uncapped.xml, /android:width="480dp"/);

  const capped = convertSvg(large, false, 200);
  assert.match(capped.xml, /android:width="200dp"/);
  assert.match(capped.xml, /android:height="133dp"/);
  assert.match(capped.xml, /android:viewportWidth="480"/);
  assert.deepEqual(capped.analysis.diagnostics, []);
  assert.equal(
    capped.analysis.metrics.estimated_xml_bytes,
    Buffer.byteLength(convertSvg(large, false, 200, true).xml, "utf8"),
  );
});

test("notification conversion whitens and fits artwork on a 24dp canvas", () => {
  const result = convertNotificationSvg(exact, true, 20);
  assert.equal(result.ok, true);
  assert.deepEqual([result.analysis.metrics.width, result.analysis.metrics.height], [24, 24]);
  assert.match(result.xml, /android:fillColor="#FFFFFF"/);
  assert.ok(result.analysis.diagnostics.some(({ code }) => code === "VDT021"));
  for (const optimize of [false, true]) {
    const compact = convertNotificationSvg(exact, optimize, 20);
    const pretty = convertNotificationSvg(exact, optimize, 20, true);
    const readableBytes = Buffer.byteLength(pretty.xml, "utf8");
    assert.equal(compact.analysis.metrics.estimated_xml_bytes, readableBytes);
    assert.equal(pretty.analysis.metrics.estimated_xml_bytes, readableBytes);
  }
});

test("resource names match the command-line interface", () => {
  assert.equal(resourceName("Icons/Arrow Left"), "icons_arrow_left");
  assert.equal(resourceName("HTTPServer"), "http_server");
  assert.equal(resourceName("class"), "ic_class");
  assert.equal(resourceName("_private"), "_private");
  assert.equal(resourceName("🙂"), undefined);
});
