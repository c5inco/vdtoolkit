import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import init, { analyzeSvg, convertSvg } from "../pkg/vdtoolkit_wasm.js";

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
    [["SVGVD016", "warning"]],
  );
  assert.match(uncapped.xml, /android:width="480dp"/);

  const capped = convertSvg(large, false, 200);
  assert.match(capped.xml, /android:width="200dp"/);
  assert.match(capped.xml, /android:height="133dp"/);
  assert.match(capped.xml, /android:viewportWidth="480"/);
  assert.deepEqual(capped.analysis.diagnostics, []);
});
