#!/usr/bin/env node
// Measure svg2vectordrawable's documented Node API on a preloaded SVG corpus.
// Pair this with `cargo run --example benchmark_convert --release` so neither
// measurement includes process startup or filesystem reads.

import { readdir, readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { performance } from 'node:perf_hooks';

const [referenceDirectory, corpusDirectory, precisionArgument = '6'] = process.argv.slice(2);
if (!referenceDirectory || !corpusDirectory || !/^(?:[1-9]|10)$/.test(precisionArgument)) {
  throw new Error('usage: benchmark-svg2vectordrawable.mjs <svg2vectordrawable-directory> <svg-directory> [precision 1..10]');
}

const precision = Number(precisionArgument);
const require = createRequire(import.meta.url);
const convert = require(resolve(referenceDirectory));
const filenames = (await readdir(corpusDirectory)).filter((filename) => filename.endsWith('.svg')).sort();
if (filenames.length === 0) throw new Error('SVG directory is empty');
const sources = await Promise.all(filenames.map((filename) => readFile(resolve(corpusDirectory, filename), 'utf8')));
const samples = [];
let outputBytes = 0;

for (let round = 0; round < 11; round++) {
  const started = performance.now();
  outputBytes = 0;
  for (const source of sources) {
    outputBytes += (await convert(source, {
      floatPrecision: precision,
      strict: false,
      fillBlack: false,
      xmlTag: false,
    })).length;
  }
  if (round > 0) samples.push(Number((performance.now() - started).toFixed(3)));
}

console.log(JSON.stringify({
  tool: 'svg2vectordrawable',
  policy: `precision-${precision}`,
  assets: sources.length,
  output_bytes: outputBytes,
  samples_ms: samples,
}));
