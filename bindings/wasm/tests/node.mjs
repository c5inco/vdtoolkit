import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import init, { analyzeSvg, convertSvg } from "../pkg/svg2vd_wasm.js";

const wasm = await readFile(new URL("../pkg/svg2vd_wasm_bg.wasm", import.meta.url));
await init({ module_or_path: wasm });

const bytes = (source) => new TextEncoder().encode(source);
const exact = bytes('<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><path d="M0 0H20V10Z" fill="#123456"/></svg>');
const normalized = bytes('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect x="1" y="2" width="3" height="4" fill="#123456"/></svg>');
const unsupported = bytes('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><text>hello</text></svg>');
const malformed = bytes("<svg><path></svg>");
const decimals = bytes('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M1.234567 2.345678L20.987654 21.876543" fill="#123456"/></svg>');

test("the generated module returns exact and normalized analysis", () => {
  assert.equal(analyzeSvg(exact).analysis.compatibility, "exact");
  const result = convertSvg(normalized, false);
  assert.equal(result.ok, true);
  assert.equal(result.analysis.compatibility, "exact_with_normalization");
  assert.match(result.xml, /android:pathData="M1,2 L4,2 L4,6 L1,6 Z"/);
});

test("unsupported and malformed inputs are structured failures", () => {
  const unsupportedResult = convertSvg(unsupported, false);
  assert.equal(unsupportedResult.ok, false);
  assert.equal(unsupportedResult.error.kind, "unsupported");
  assert.equal(unsupportedResult.error.analysis.compatibility, "unsupported");
  assert.ok(unsupportedResult.error.analysis.diagnostics.some(({ code }) => code === "SVGVD006"));

  const malformedResult = convertSvg(malformed, false);
  assert.equal(malformedResult.ok, false);
  assert.equal(malformedResult.error.kind, "xml");
  assert.equal("analysis" in malformedResult.error, false);
});

test("optimization is effective and output is deterministic", () => {
  const plain = convertSvg(decimals, false);
  const optimized = convertSvg(decimals, true);
  assert.ok(optimized.xml.length < plain.xml.length);
  assert.match(optimized.xml, /M1\.235,2\.346/);
  assert.equal(optimized.analysis.metrics.estimated_xml_bytes, optimized.xml.length);
  assert.deepEqual(convertSvg(exact, false), convertSvg(exact, false));
});
