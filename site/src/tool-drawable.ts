// Drawable tool: convert one SVG and compare how three Androids draw it.

import type { CanvasKit } from "canvaskit-wasm";
import { convertSvg } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm.js";
import type { Analysis, ConvertResult } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";
import type { AndroidVersion } from "./android/render.ts";
import { renderColumns } from "./columns.ts";
import { resourceName, showXml } from "./xml-pane.ts";
import nestedClips from "../samples/nested-clips.svg";
import mask from "../samples/mask.svg";
import radialGradient from "../samples/radial-gradient.svg";
import stopwatch from "../samples/stopwatch.svg";

const SAMPLES = [
  { slug: "nested-clips", name: "Nested clips", source: nestedClips },
  { slug: "mask", name: "Mask", source: mask },
  { slug: "radial-gradient", name: "Radial gradient", source: radialGradient },
  { slug: "stopwatch", name: "Stopwatch", source: stopwatch },
];

// Why a version disagrees, in the words a developer needs.
const REASONS: Record<string, Partial<Record<AndroidVersion, string>>> = {
  "nested-clips": { api21: "Android 5.0 never restores a clip when its group ends, so the green bar is cut by the inner clip too." },
  "mask": { api21: "Android 5.0 replaces the clip instead of intersecting, so only the second clip applies." },
};

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

interface Loaded {
  name: string;
  slug: string | null;
  xml: string | null;
  analysis: Analysis | null;
  error: string | null;
}

export class DrawableTool {
  readonly id = "drawable";
  readonly section = $("tool-drawable");
  private readonly el = {
    fileName: $("file-name"),
    samples: $("samples"),
    density: $<HTMLSelectElement>("density"),
    zoom: $<HTMLInputElement>("zoom"),
    zoomValue: $<HTMLOutputElement>("zoom-value"),
    columns: $("columns"),
  };
  private loaded: Loaded | null = null;
  private readonly encoder = new TextEncoder();
  private readonly ck: CanvasKit;

  constructor(ck: CanvasKit) {
    this.ck = ck;
    for (const sample of SAMPLES) {
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.slug = sample.slug;
      button.textContent = sample.name;
      button.addEventListener("click", () => {
        history.replaceState(null, "", `#${sample.slug}`);
        this.load(sample.name, sample.source, sample.slug);
      });
      this.el.samples.append(button);
    }
    this.el.density.addEventListener("change", () => this.render());
    this.el.zoom.addEventListener("input", () => this.render());
  }

  /** Shows the tool; `hash` may name a sample. */
  activate(hash: string): void {
    const linked = SAMPLES.find((sample) => `#${sample.slug}` === hash);
    if (linked) this.load(linked.name, linked.source, linked.slug);
    else if (!this.loaded) this.load(SAMPLES[0].name, SAMPLES[0].source, SAMPLES[0].slug);
    else this.show();
  }

  async open(file: File): Promise<void> {
    history.replaceState(null, "", location.pathname);
    this.load(file.name, await file.text());
  }

  private load(name: string, text: string, slug: string | null = null): void {
    this.el.fileName.textContent = name.replace(/\.(svg|xml)$/i, "");
    if (/<vector[\s>]/.test(text)) {
      this.loaded = { name, slug, xml: text, analysis: null, error: null };
    } else {
      const result = convertSvg(this.encoder.encode(text), false, undefined, true) as ConvertResult;
      this.loaded = result.ok
        ? { name, slug, xml: result.xml, analysis: result.analysis, error: null }
        : { name, slug, xml: null, analysis: result.error.analysis ?? null, error: result.error.message };
    }
    for (const button of this.el.samples.querySelectorAll("button")) {
      button.setAttribute("aria-pressed", String(button.dataset.slug === slug));
    }
    this.show();
  }

  private show(): void {
    if (!this.loaded) return;
    showXml({
      heading: "VectorDrawable XML",
      xml: this.loaded.xml,
      placeholder: this.loaded.error ?? "",
      analysis: this.loaded.analysis,
      downloadName: `${resourceName(this.loaded.name)}.xml`,
    });
    this.render();
  }

  private render(): void {
    if (!this.loaded) return;
    const zoom = Number(this.el.zoom.value);
    this.el.zoomValue.value = `${zoom}×`;
    renderColumns(this.el.columns, this.ck, {
      name: this.loaded.name,
      xml: this.loaded.xml,
      error: this.loaded.error,
      minimumApi: this.loaded.analysis?.minimum_api ?? null,
      reasons: this.loaded.slug ? REASONS[this.loaded.slug] : undefined,
      density: Number(this.el.density.value),
      zoom,
    });
  }
}
