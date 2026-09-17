import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  ICON_DP, LAYER_DP, LAYER_SCALE, LEGACY_DP, MASKS, placeIcon, SAFE_ZONE_DP,
} from "../src/icon-geometry.ts";

const ROOT = new URL("../..", import.meta.url).pathname;

test("the geometry constants match the Rust library", () => {
  const source = readFileSync(join(ROOT, "src/adaptive.rs"), "utf8");
  const constant = (name: string) => Number(new RegExp(`pub const ${name}: f32 = ([\\d.]+);`).exec(source)?.[1]);
  assert.equal(LAYER_DP, constant("ADAPTIVE_ICON_SIZE"));
  assert.equal(SAFE_ZONE_DP, constant("ADAPTIVE_ICON_SAFE_ZONE"));
  assert.equal(ICON_DP, constant("ADAPTIVE_ICON_VISIBLE_DIAMETER"));
  assert.equal(LEGACY_DP, constant("LEGACY_ICON_SIZE"));
});

test("a 72dp icon shows its 108dp layers unscaled", () => {
  assert.equal(LAYER_DP / ICON_DP, LAYER_SCALE);
  const placement = placeIcon(72 * 3);
  assert.equal(placement.layerSize, 108 * 3);
  assert.equal(placement.layerOffset, -18 * 3);
  assert.equal(placement.safeZone, 66 * 3);
  assert.equal(placement.maskScale, 2.16);
});

test("every mask is a closed path on the 100 unit square", () => {
  for (const mask of MASKS) {
    assert.match(mask.path, /Z$/i, mask.id);
    const numbers = mask.path.match(/-?\d+(\.\d+)?/g)!.map(Number);
    assert.ok(numbers.every((n) => n >= 0 && n <= 100), mask.id);
  }
});
