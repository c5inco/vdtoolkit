// Checks the ports against pixels verified on devices. Expected values come
// from tools/android-renderer/test/.../RendererConformanceTest.java (API 24,
// 34, and 36 on emulator.wtf) and the API 21 findings in
// tools/android-renderer/RESULTS.md.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { before, describe, test } from "node:test";
import { DOMParser } from "@xmldom/xmldom";
import CanvasKitInit, { type CanvasKit } from "canvaskit-wasm";
import { createNodesFromPathData, nodesToPath, type PathSink } from "../src/android/path-data-api21.ts";
import { getPathDataFromAsciiString } from "../src/android/path-data-hwui.ts";
import {
  DrawableLoadError, renderVectorDrawable, type AndroidVersion, type RenderedDrawable,
} from "../src/android/render.ts";
import type { XmlElement } from "../src/android/vector-xml.ts";

const ROOT = new URL("../..", import.meta.url).pathname;
const FIXTURES = join(ROOT, "tools/android-renderer/fixtures");
const SIZE = 240;
const TOLERANCE = 2;

let ck: CanvasKit;
const xml = new Map<string, string>();

before(async () => {
  ck = await CanvasKitInit();
  const out = mkdtempSync(join(tmpdir(), "vdt-site-"));
  execFileSync("cargo", ["build", "--quiet", "--manifest-path", join(ROOT, "Cargo.toml")], { stdio: "inherit" });
  for (const name of FIXTURES_USED) {
    const target = join(out, `${name}.xml`);
    execFileSync(join(ROOT, "target/debug/vdt"), ["convert", join(FIXTURES, `${name}.svg`), "-o", target]);
    xml.set(name, readFileSync(target, "utf8"));
  }
});

const FIXTURES_USED = [
  "transformed_clip", "nested_clip_scope", "hard_white_mask_region", "even_odd",
  "skewed_linear_gradient", "radial_gradient",
];

function render(name: string, version: AndroidVersion): RenderedDrawable {
  const document = new DOMParser().parseFromString(xml.get(name)!, "text/xml");
  return renderVectorDrawable(ck, document.documentElement as unknown as XmlElement, {
    version,
    bounds: { width: SIZE, height: SIZE },
  });
}

function pixel(bitmap: RenderedDrawable, x: number, y: number): number {
  const i = (y * bitmap.width + x) * 4;
  const [r, g, b, a] = bitmap.pixels.subarray(i, i + 4);
  return ((a << 24) | (r << 16) | (g << 8) | b) >>> 0;
}

function assertPixel(bitmap: RenderedDrawable, x: number, y: number, expected: number): void {
  const actual = pixel(bitmap, x, y);
  for (const shift of [24, 16, 8, 0]) {
    const delta = Math.abs(((actual >>> shift) & 0xff) - ((expected >>> shift) & 0xff));
    assert.ok(delta <= TOLERANCE,
      `pixel at ${x},${y}: expected ${expected.toString(16)}, actual ${actual.toString(16)}`);
  }
}

function assertTransparent(bitmap: RenderedDrawable, x: number, y: number): void {
  const alpha = pixel(bitmap, x, y) >>> 24;
  assert.ok(alpha <= TOLERANCE, `expected transparent pixel at ${x},${y}, alpha=${alpha}`);
}

for (const version of ["api21", "api24", "latest"] as const) {
  describe(`${version} matches device pixels`, () => {
    test("transformed clip", () => {
      const bitmap = render("transformed_clip", version);
      assertPixel(bitmap, 70, 90, 0xff1267d6);
      assertPixel(bitmap, 130, 160, 0xff1267d6);
      assertTransparent(bitmap, 50, 90);
      assertTransparent(bitmap, 70, 180);
    });
  });
}

for (const version of ["api24", "latest"] as const) {
  describe(`${version} matches device pixels`, () => {
    test("nested clip intersection and scope", () => {
      const bitmap = render("nested_clip_scope", version);
      assertPixel(bitmap, 70, 30, 0xffc52a54);
      assertTransparent(bitmap, 170, 30);
      assertPixel(bitmap, 30, 160, 0xff159a55);
      assertTransparent(bitmap, 220, 160);
    });

    test("hard white mask and mask region", () => {
      const bitmap = render("hard_white_mask_region", version);
      assertPixel(bitmap, 50, 60, 0xffe37a19);
      assertPixel(bitmap, 150, 140, 0xffe37a19);
      assertTransparent(bitmap, 170, 140);
      assertTransparent(bitmap, 80, 160);
    });

    test("even-odd fill", () => {
      const bitmap = render("even_odd", version);
      assertPixel(bitmap, 30, 30, 0xff713bc1);
      assertTransparent(bitmap, 80, 80);
      assertPixel(bitmap, 190, 170, 0xff713bc1);
      assertTransparent(bitmap, 230, 120);
    });

    test("skewed linear gradient", () => {
      const bitmap = render("skewed_linear_gradient", version);
      assertPixel(bitmap, 40, 40, 0xff1267d6);
      assertPixel(bitmap, 220, 40, 0xff1267d6);
      assertPixel(bitmap, 20, 200, 0xffe37a19);
      assertPixel(bitmap, 200, 200, 0xffe37a19);
    });

    test("scaled circular radial gradient", () => {
      const bitmap = render("radial_gradient", version);
      assertPixel(bitmap, 120, 120, 0xff159a55);
      assertPixel(bitmap, 180, 120, 0xffc52a54);
      assertPixel(bitmap, 120, 60, 0xffc52a54);
      assertPixel(bitmap, 20, 20, 0xffc52a54);
    });
  });
}

describe("api21 reproduces Android 5.0 behavior found on devices", () => {
  test("a nested group clip stays active for a following sibling", () => {
    const bitmap = render("nested_clip_scope", "api21");
    assertPixel(bitmap, 70, 30, 0xffc52a54);
    assertTransparent(bitmap, 30, 160); // green on API 24 and later
  });

  test("a second clip replaces the first instead of intersecting", () => {
    const bitmap = render("hard_white_mask_region", "api21");
    assertPixel(bitmap, 50, 60, 0xffe37a19);
    assertPixel(bitmap, 170, 140, 0xffe37a19); // transparent on API 24 and later
  });

  test("gradient colors fail to load", () => {
    assert.throws(() => render("radial_gradient", "api21"), DrawableLoadError);
  });

  test("even-odd fill is ignored", () => {
    const bitmap = render("even_odd", "api21");
    assertPixel(bitmap, 80, 80, 0xff713bc1); // the hole is filled
  });
});

describe("path data parsers", () => {
  test("Android 5.0 cannot split 1.5.5 or read exponents", () => {
    assert.throws(() => createNodesFromPathData("M1.5.5 L2 2"), DrawableLoadError);
    // `e` starts a new command, leaving M one number short when drawn.
    const nodes = createNodesFromPathData("M1e-5 0 L2 2");
    assert.deepEqual(nodes.map((node) => [node.type, node.params]), [
      ["M", [1]], ["e", [-5, 0]], ["L", [2, 2]],
    ]);
    const sink = new Proxy({}, { get: () => () => {} }) as PathSink;
    assert.throws(() => nodesToPath(nodes, sink), DrawableLoadError);
    assert.deepEqual(createNodesFromPathData("M1-2l3,4z").map((node) => [node.type, node.params]), [
      ["M", [1, -2]], ["l", [3, 4]], ["z", []],
    ]);
  });

  test("the native parser splits 1.5.5 and reads exponents", () => {
    assert.deepEqual(getPathDataFromAsciiString("M1.5.5 L1e-1-2").points, [1.5, 0.5, Math.fround(0.1), -2]);
    assert.throws(() => getPathDataFromAsciiString("M1 2 3"), DrawableLoadError);
  });
});

describe("arcs", () => {
  const circle = `<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">
    <path android:fillColor="#1267D6" android:pathData="M12,2 A10,10 0 1,1 12,22 A10,10 0 1,1 12,2 Z"/>
    <path android:strokeColor="#E37A19" android:strokeWidth="1" android:pathData="M4,12 a8,8 0 0,0 16,0"/>
  </vector>`;

  for (const version of ["api21", "api24", "latest"] as const) {
    test(`${version} draws arcs in place`, () => {
      const document = new DOMParser().parseFromString(circle, "text/xml");
      const bitmap = renderVectorDrawable(ck, document.documentElement as unknown as XmlElement, {
        version,
        bounds: { width: SIZE, height: SIZE },
      });
      assertPixel(bitmap, 120, 60, 0xff1267d6); // inside the circle
      assertTransparent(bitmap, 30, 30); // outside the circle
      assertPixel(bitmap, 120, 200, 0xffe37a19); // bottom of the stroked half circle
    });
  }
});
