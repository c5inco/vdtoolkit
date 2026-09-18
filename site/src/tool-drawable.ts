// Drawable tool: convert one SVG and compare how Android 7 and the latest Android draw it.

import type { CanvasKit } from "canvaskit-wasm";
import { convertSvg } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm.js";
import type { Analysis, ConvertResult } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";
import type { AndroidVersion } from "./android/render.ts";
import { renderColumns } from "./columns.ts";
import { resourceName, showXml } from "./xml-pane.ts";
import alphaGradient from "../samples/alpha-gradient.svg";
import angledClip from "../samples/angled-clip.svg";
import scaledStroke from "../samples/scaled-stroke.xml";
import stopwatch from "../samples/stopwatch.svg";

// Each sample exists because it exposes one way Android 7's renderer loses to
// the current one — except the stopwatch, the control that draws the same.
const SAMPLES = [
  { slug: "alpha-gradient", name: "Alpha gradient", source: alphaGradient },
  { slug: "angled-clip", name: "Angled clip", source: angledClip },
  { slug: "scaled-stroke", name: "Scaled stroke", source: scaledStroke },
  { slug: "stopwatch", name: "Stopwatch", source: stopwatch },
];

// Why Android 7 disagrees, in the words a developer needs.
const REASONS: Record<string, Partial<Record<AndroidVersion, string>>> = {
  "alpha-gradient": { api24: "Android 7 mixes gradient stops without premultiplying alpha, so the fade passes through a darker, muddier color." },
  "angled-clip": { api24: "Android 7 does not antialias clip edges, so the diagonal seam is jagged." },
  "scaled-stroke": { api24: "Android 7 scales a stroke by the group's smaller axis, so the ring stays 2dp all around instead of thickening at the sides." },
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
      reasons: this.loaded.slug ? REASONS[this.loaded.slug] : undefined,
      density: Number(this.el.density.value),
      zoom,
    });
  }
}
