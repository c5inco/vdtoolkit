import CanvasKitInit, { type CanvasKit } from "canvaskit-wasm";
import initVdt, { convertSvg } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm.js";
import type { Analysis, ConvertResult } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";
import vdtWasm from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm_bg.wasm";
import {
  ANDROID_VERSIONS, DrawableLoadError, renderVectorDrawable, type AndroidVersion, type RenderedDrawable,
} from "./android/render.ts";
import type { XmlElement } from "./android/vector-xml.ts";
import nestedClips from "../samples/nested-clips.svg";
import mask from "../samples/mask.svg";
import radialGradient from "../samples/radial-gradient.svg";
import stopwatch from "../samples/stopwatch.svg";

const SAMPLES = [
  { file: "nested-clips.svg", name: "Nested clips", note: "API 21 keeps a clip active after its group ends.", source: nestedClips },
  { file: "mask.svg", name: "Mask", note: "Two clips: API 21 keeps only the second.", source: mask },
  { file: "radial-gradient.svg", name: "Radial gradient", note: "Gradients need API 24.", source: radialGradient },
  { file: "stopwatch.svg", name: "Stopwatch icon", note: "Strokes and arcs that work everywhere.", source: stopwatch },
];

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const elements = {
  drop: $("drop"),
  file: $<HTMLInputElement>("file"),
  fileName: $("file-name"),
  samples: $("samples"),
  density: $<HTMLSelectElement>("density"),
  zoom: $<HTMLInputElement>("zoom"),
  zoomValue: $<HTMLOutputElement>("zoom-value"),
  columns: $("columns"),
  summary: $("summary"),
  diagnostics: $("diagnostics"),
  xml: $("xml"),
  copy: $<HTMLButtonElement>("copy"),
  download: $<HTMLButtonElement>("download"),
};

interface Loaded {
  name: string;
  xml: string | null;
  analysis: Analysis | null;
  error: string | null;
}

let ck: CanvasKit;
let loaded: Loaded | null = null;

const encoder = new TextEncoder();

function load(name: string, text: string): void {
  elements.fileName.textContent = name;
  if (/<vector[\s>]/.test(text)) {
    loaded = { name, xml: text, analysis: null, error: null };
  } else {
    const result = convertSvg(encoder.encode(text), false, undefined, true) as ConvertResult;
    loaded = result.ok
      ? { name, xml: result.xml, analysis: result.analysis, error: null }
      : { name, xml: null, analysis: result.error.analysis ?? null, error: result.error.message };
  }
  for (const button of elements.samples.querySelectorAll("button")) {
    button.setAttribute("aria-pressed", String(button.dataset.file === name));
  }
  showOutput();
  renderAll();
}

function showOutput(): void {
  if (!loaded) return;
  const { xml, analysis, error } = loaded;
  elements.xml.textContent = xml ?? error ?? "";
  elements.copy.disabled = elements.download.disabled = xml === null;

  elements.summary.replaceChildren();
  if (analysis) {
    const facts: [string, string][] = [
      ["Minimum API", analysis.minimum_api === null ? "not convertible" : String(analysis.minimum_api)],
      ["Result", COMPATIBILITY[analysis.compatibility]],
      ["Paths", String(analysis.metrics.paths)],
    ];
    if (xml) facts.push(["Size", `${encoder.encode(xml).length.toLocaleString()} bytes`]);
    for (const [label, value] of facts) {
      const span = document.createElement("span");
      span.append(`${label} `, Object.assign(document.createElement("strong"), { textContent: value }));
      elements.summary.append(span);
    }
  }

  elements.diagnostics.replaceChildren(
    ...(analysis?.diagnostics ?? []).map((diagnostic) => {
      const item = document.createElement("li");
      item.className = diagnostic.severity;
      const code = Object.assign(document.createElement("span"), { className: "code", textContent: diagnostic.code });
      item.append(code, diagnostic.message);
      return item;
    }),
  );
}

const COMPATIBILITY: Record<Analysis["compatibility"], string> = {
  exact: "exact",
  exact_with_normalization: "exact, after normalizing",
  approximate: "approximate",
  unsupported: "unsupported",
};

function renderAll(): void {
  elements.columns.replaceChildren();
  if (!loaded) return;
  const density = Number(elements.density.value);
  const zoom = Number(elements.zoom.value);
  elements.zoomValue.value = `${zoom}x`;

  const parsed = loaded.xml === null ? null : new DOMParser().parseFromString(loaded.xml, "application/xml");
  const parseError = parsed?.querySelector("parsererror")?.textContent ?? null;

  const results = new Map<AndroidVersion, RenderedDrawable | Error>();
  for (const { id } of ANDROID_VERSIONS) {
    if (!parsed || parseError) {
      results.set(id, new DrawableLoadError(parseError ? "The XML is not well formed" : loaded.error ?? "Nothing to draw"));
      continue;
    }
    try {
      results.set(id, renderVectorDrawable(ck, parsed.documentElement as unknown as XmlElement, { version: id, density }));
    } catch (error) {
      results.set(id, error instanceof Error ? error : new Error(String(error)));
    }
  }

  const latest = results.get("latest");
  for (const version of ANDROID_VERSIONS) {
    const result = results.get(version.id)!;
    const column = document.createElement("div");
    column.className = "column";
    const head = document.createElement("div");
    head.className = "column-head";
    head.append(
      Object.assign(document.createElement("span"), { className: "column-title", textContent: version.label }),
      Object.assign(document.createElement("span"), { className: "column-source", textContent: version.source }),
    );
    const stage = document.createElement("div");
    stage.className = "stage";
    column.append(head, stage);

    if (result instanceof Error) {
      column.dataset.state = "error";
      stage.append(Object.assign(document.createElement("span"), { className: "size", textContent: "Not drawn" }));
      column.append(note("error", result instanceof DrawableLoadError
        ? `Fails to load: ${result.message}.`
        : `The preview failed: ${result.message}`));
    } else {
      const canvas = document.createElement("canvas");
      canvas.width = result.width;
      canvas.height = result.height;
      canvas.style.width = `${(result.width * zoom) / devicePixelRatio}px`;
      canvas.style.height = `${(result.height * zoom) / devicePixelRatio}px`;
      canvas.setAttribute("role", "img");
      canvas.setAttribute("aria-label", `${loaded.name} drawn as ${version.label}`);
      canvas.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(result.pixels), result.width, result.height), 0, 0);
      stage.append(canvas);
      column.append(Object.assign(document.createElement("span"), {
        className: "size",
        textContent: `${result.width} × ${result.height} px`,
      }));
      if (version.id !== "latest" && !(latest instanceof Error) && latest && differs(result, latest)) {
        column.dataset.state = "differs";
        column.append(note("differs", "Draws differently from the latest Android."));
      }
    }

    const minimumApi = loaded.analysis?.minimum_api ?? null;
    if (version.id === "api21" && minimumApi !== null && minimumApi > 21) {
      column.append(note("not-loaded",
        `This file needs API ${minimumApi}, so API 21 to ${minimumApi - 1} never load it: put it in ` +
        `drawable-v${minimumApi} with a fallback. ${result instanceof Error ? "" : "Shown is what API 21 would draw if it did."}`));
    }
    elements.columns.append(column);
  }
}

function note(kind: string, text: string): HTMLParagraphElement {
  return Object.assign(document.createElement("p"), { className: `note ${kind}`, textContent: text });
}

// Ignores edge smoothing: counts pixels whose color or opacity moved by more than a quarter.
function differs(a: RenderedDrawable, b: RenderedDrawable): boolean {
  if (a.width !== b.width || a.height !== b.height) return false;
  let changed = 0;
  for (let i = 0; i < a.pixels.length; i += 4) {
    const delta = Math.max(
      Math.abs(a.pixels[i + 3] - b.pixels[i + 3]),
      Math.abs(a.pixels[i] - b.pixels[i]) * Math.min(a.pixels[i + 3], b.pixels[i + 3]) / 255,
      Math.abs(a.pixels[i + 1] - b.pixels[i + 1]) * Math.min(a.pixels[i + 3], b.pixels[i + 3]) / 255,
      Math.abs(a.pixels[i + 2] - b.pixels[i + 2]) * Math.min(a.pixels[i + 3], b.pixels[i + 3]) / 255,
    );
    if (delta > 64) changed++;
  }
  return changed > (a.width * a.height) / 200;
}

async function main(): Promise<void> {
  [ck] = await Promise.all([
    CanvasKitInit({ locateFile: (file: string) => new URL(file, import.meta.url).href }),
    initVdt({ module_or_path: new URL(vdtWasm as unknown as string, import.meta.url) }),
  ]);

  for (const sample of SAMPLES) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "sample";
    button.dataset.file = sample.file;
    button.append(
      Object.assign(document.createElement("span"), { className: "sample-name", textContent: sample.name }),
      Object.assign(document.createElement("span"), { className: "sample-note", textContent: sample.note }),
    );
    button.addEventListener("click", () => {
      history.replaceState(null, "", `#${sample.file.replace(".svg", "")}`);
      load(sample.file, sample.source);
    });
    elements.samples.append(button);
  }

  const readFile = async (file: File | undefined) => {
    if (file) load(file.name, await file.text());
  };
  elements.file.addEventListener("change", () => readFile(elements.file.files?.[0]));
  elements.drop.addEventListener("dragover", (event) => {
    event.preventDefault();
    elements.drop.classList.add("dragging");
  });
  elements.drop.addEventListener("dragleave", () => elements.drop.classList.remove("dragging"));
  elements.drop.addEventListener("drop", (event) => {
    event.preventDefault();
    elements.drop.classList.remove("dragging");
    readFile(event.dataTransfer?.files[0]);
  });

  elements.density.addEventListener("change", renderAll);
  elements.zoom.addEventListener("input", renderAll);
  elements.copy.addEventListener("click", async () => {
    if (!loaded?.xml) return;
    await navigator.clipboard.writeText(loaded.xml);
    elements.copy.textContent = "Copied";
    setTimeout(() => (elements.copy.textContent = "Copy XML"), 1500);
  });
  elements.download.addEventListener("click", () => {
    if (!loaded?.xml) return;
    const link = document.createElement("a");
    link.href = URL.createObjectURL(new Blob([loaded.xml], { type: "text/xml" }));
    link.download = `${loaded.name.replace(/\.[^.]+$/, "").replace(/[^a-z0-9_]+/gi, "_").toLowerCase()}.xml`;
    link.click();
    URL.revokeObjectURL(link.href);
  });

  const linked = SAMPLES.find((sample) => `#${sample.file.replace(".svg", "")}` === location.hash) ?? SAMPLES[0];
  load(linked.file, linked.source);
}

main().catch((error) => {
  elements.columns.replaceChildren(note("error", `The page could not start: ${error instanceof Error ? error.message : error}`));
});
