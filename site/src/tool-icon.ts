// Launcher icon tool: `vdt adaptive` in the browser. Change a flag, watch the
// files and the icon change.

import type { CanvasKit } from "canvaskit-wasm";
import { adaptiveIcon } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm.js";
import type { AdaptiveIconFile, AdaptiveIconResult, Analysis } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";
import { createZip } from "../../figma-plugin/src/zip.ts";
import { renderVectorDrawable, type RenderedDrawable } from "./android/render.ts";
import { note, parseVector, pixelCanvas, renderColumns } from "./columns.ts";
import { ICON_DP, LEGACY_DP, MASKS, placeIcon } from "./icon-geometry.ts";
import { downloadBlob, showXml } from "./xml-pane.ts";
import foregroundSample from "../samples/launcher-foreground.svg";
import monochromeSample from "../samples/launcher-monochrome.svg";
import backgroundSample from "../samples/launcher-background.svg";
import gradientSample from "../samples/launcher-gradient.svg";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

interface LayerSource {
  name: string;
  text: string;
}

type LayerId = "foreground" | "background" | "monochrome";

const LEGACY_DENSITIES: [string, number][] = [["mdpi", 1], ["hdpi", 1.5], ["xhdpi", 2], ["xxhdpi", 3], ["xxxhdpi", 4]];

export class IconTool {
  readonly id = "icon";
  readonly section = $("tool-icon");
  private readonly el = {
    form: $<HTMLFormElement>("icon-form"),
    names: { foreground: $("fg-name"), background: $("bg-name"), monochrome: $("mono-name") },
    notes: { foreground: $("fg-note"), background: $("bg-note"), monochrome: $("mono-note") },
    files: { foreground: $<HTMLInputElement>("fg-file"), background: $<HTMLInputElement>("bg-file"), monochrome: $<HTMLInputElement>("mono-file") },
    bgKind: () => (this.el.form.elements.namedItem("bg-kind") as RadioNodeList).value as "color" | "svg",
    bgColor: $<HTMLInputElement>("bg-color"),
    bgHex: $<HTMLInputElement>("bg-hex"),
    monoOn: $<HTMLInputElement>("mono-on"),
    fit: $<HTMLInputElement>("fit"),
    fitValue: $<HTMLOutputElement>("fit-value"),
    legacy: $<HTMLInputElement>("legacy"),
    optimize: $<HTMLInputElement>("optimize"),
    name: $<HTMLInputElement>("icon-name"),
    safeZone: $<HTMLInputElement>("safe-zone"),
    density: $<HTMLSelectElement>("icon-density"),
    zoom: $<HTMLInputElement>("icon-zoom"),
    zoomValue: $<HTMLOutputElement>("icon-zoom-value"),
    command: $("command"),
    previews: $("previews"),
    fileList: $("files"),
    zip: $<HTMLButtonElement>("zip"),
    legacyBlock: $("legacy-block"),
    legacyColumns: $("icon-columns"),
  };
  private layers: Record<LayerId, LayerSource> = {
    foreground: { name: "launcher-foreground.svg", text: foregroundSample },
    background: { name: "launcher-background.svg", text: backgroundSample },
    monochrome: { name: "launcher-monochrome.svg", text: monochromeSample },
  };
  private result: AdaptiveIconResult | null = null;
  private selectedFile: string | null = null;
  private readonly encoder = new TextEncoder();
  private readonly ck: CanvasKit;

  constructor(ck: CanvasKit) {
    this.ck = ck;
    for (const layer of ["foreground", "background", "monochrome"] as const) {
      this.el.files[layer].addEventListener("change", async () => {
        const file = this.el.files[layer].files?.[0];
        if (file) this.setLayer(layer, { name: file.name, text: await file.text() });
      });
    }
    for (const button of this.section.querySelectorAll<HTMLButtonElement>("button[data-sample]")) {
      button.addEventListener("click", () => {
        const [layer, which] = button.dataset.sample!.split(":") as [LayerId, string];
        const samples: Record<string, LayerSource> = {
          foreground: { name: "launcher-foreground.svg", text: foregroundSample },
          monochrome: { name: "launcher-monochrome.svg", text: monochromeSample },
          flat: { name: "launcher-background.svg", text: backgroundSample },
          gradient: { name: "launcher-gradient.svg", text: gradientSample },
        };
        if (layer === "background") (this.el.form.elements.namedItem("bg-kind") as RadioNodeList).value = "svg";
        this.setLayer(layer, samples[which]);
      });
    }
    this.el.bgColor.addEventListener("input", () => {
      this.el.bgHex.value = this.el.bgColor.value.toUpperCase();
      this.update();
    });
    this.el.bgHex.addEventListener("change", () => {
      if (/^#[0-9a-f]{6}$/i.test(this.el.bgHex.value)) this.el.bgColor.value = this.el.bgHex.value.toLowerCase();
      this.update();
    });
    this.el.form.addEventListener("input", (event) => {
      if (event.target === this.el.bgColor || event.target === this.el.bgHex) return;
      this.update();
    });
    this.el.form.addEventListener("submit", (event) => event.preventDefault());
    this.el.safeZone.addEventListener("change", () => this.drawPreviews());
    this.el.density.addEventListener("change", () => this.drawAll());
    this.el.zoom.addEventListener("input", () => this.drawAll());
    this.el.zip.addEventListener("click", () => this.downloadZip());
  }

  activate(): void {
    if (this.result) this.showSelected();
    else this.update();
  }

  async open(file: File): Promise<void> {
    this.setLayer("foreground", { name: file.name, text: await file.text() });
  }

  private setLayer(layer: LayerId, source: LayerSource): void {
    this.layers[layer] = source;
    if (layer === "monochrome") this.el.monoOn.checked = true;
    this.update();
  }

  private options() {
    const fit = Number(this.el.fit.value);
    const useColor = this.el.bgKind() === "color";
    return {
      fit,
      legacy: this.el.legacy.checked,
      optimize: this.el.optimize.checked,
      name: this.el.name.value.trim() || "ic_launcher",
      backgroundColor: useColor ? this.el.bgHex.value.trim() : undefined,
      background: useColor ? undefined : this.layers.background,
      monochrome: this.el.monoOn.checked ? this.layers.monochrome : undefined,
    };
  }

  private update(): void {
    const options = this.options();
    this.el.fitValue.value = `${options.fit} dp`;
    for (const layer of ["foreground", "background", "monochrome"] as const) {
      this.el.names[layer].textContent = this.layers[layer].name;
      this.el.notes[layer].replaceChildren();
    }

    this.result = adaptiveIcon(
      this.encoder.encode(this.layers.foreground.text),
      options.background ? this.encoder.encode(options.background.text) : undefined,
      options.backgroundColor,
      options.monochrome ? this.encoder.encode(options.monochrome.text) : undefined,
      { fit: options.fit, legacy: options.legacy, optimize: options.optimize, name: options.name, pretty: true },
    ) as AdaptiveIconResult;

    this.el.command.textContent = [
      "vdt adaptive",
      `--foreground ${this.layers.foreground.name}`,
      options.background ? `--background ${options.background.name}` : `--background-color '${options.backgroundColor}'`,
      options.monochrome ? `--monochrome ${options.monochrome.name}` : "",
      options.fit !== 108 ? `--fit ${options.fit}` : "",
      options.legacy ? "--legacy" : "",
      options.optimize ? "--optimize" : "",
      options.name !== "ic_launcher" ? `--name ${options.name}` : "",
      "-o res",
    ].filter(Boolean).join(" ");

    if (!this.result.ok) {
      const layer = this.result.layer ?? "foreground";
      this.el.notes[layer].append(note("error", `${this.result.error.message}.`));
      this.el.previews.replaceChildren(note("error", "Nothing generated until every layer converts."));
      this.el.fileList.replaceChildren();
      this.el.zip.disabled = true;
      this.el.legacyBlock.hidden = true;
      showXml({ heading: "Files", xml: null, placeholder: "", downloadName: "" });
      return;
    }

    const { layers } = this.result;
    for (const [layer, analysis] of Object.entries(layers) as [LayerId | "legacy", Analysis][]) {
      if (layer === "legacy") continue;
      const warnings = analysis.diagnostics.filter((d) => d.severity !== "info");
      if (warnings.length) {
        this.el.notes[layer].append(note("differs", warnings.map((d) => d.message).join(". ") + "."));
      }
    }

    this.el.zip.disabled = false;
    this.listFiles();
    const stillThere = this.result.files.some((file) => file.path === this.selectedFile);
    this.selectFile(stillThere ? this.selectedFile! : this.result.files[0].path);
    this.drawAll();
  }

  private drawAll(): void {
    this.el.zoomValue.value = `${this.el.zoom.value}×`;
    this.drawPreviews();
    this.drawLegacy();
  }

  private listFiles(): void {
    if (!this.result?.ok) return;
    this.el.fileList.replaceChildren(...this.result.files.map((file) => {
      const item = document.createElement("li");
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.path = file.path;
      button.append(
        Object.assign(document.createElement("span"), { className: "file-dir", textContent: file.path.slice(0, file.path.lastIndexOf("/") + 1) }),
        Object.assign(document.createElement("span"), { className: "file-base", textContent: file.path.slice(file.path.lastIndexOf("/") + 1) }),
      );
      button.addEventListener("click", () => this.selectFile(file.path));
      item.append(button);
      return item;
    }));
  }

  private selectFile(path: string): void {
    this.selectedFile = path;
    for (const button of this.el.fileList.querySelectorAll<HTMLButtonElement>("button")) {
      button.setAttribute("aria-pressed", String(button.dataset.path === path));
    }
    this.showSelected();
  }

  private showSelected(): void {
    if (!this.result?.ok) return;
    const file = this.result.files.find((f) => f.path === this.selectedFile) ?? this.result.files[0];
    const analysis = this.analysisFor(file);
    showXml({
      heading: `res/${file.path}`,
      xml: file.xml ?? null,
      placeholder: file.rendered_from ? `A PNG rendered from ${file.rendered_from} for devices below API 24. Download res.zip to get it.` : "",
      analysis,
      downloadName: file.path.slice(file.path.lastIndexOf("/") + 1),
    });
  }

  private analysisFor(file: AdaptiveIconFile): Analysis | null {
    if (!this.result?.ok) return null;
    const { layers } = this.result;
    if (file.path.endsWith("_foreground.xml")) return layers.foreground;
    if (file.path.endsWith("_background.xml") && file.path.startsWith("drawable")) return layers.background ?? null;
    if (file.path.endsWith("_monochrome.xml")) return layers.monochrome ?? null;
    if (file.path.startsWith("mipmap/") || file.path.startsWith("mipmap-anydpi-v24/")) return layers.legacy ?? null;
    return null;
  }

  private layerPixels(suffix: string, density: number): RenderedDrawable | null {
    if (!this.result?.ok) return null;
    const file = this.result.files.find((f) => f.path.startsWith("drawable") && f.path.endsWith(suffix));
    if (!file?.xml) return null;
    const { root } = parseVector(file.xml);
    if (!root) return null;
    try {
      return renderVectorDrawable(this.ck, root, { version: "latest", density });
    } catch {
      return null;
    }
  }

  private drawPreviews(): void {
    if (!this.result?.ok) return;
    const density = Number(this.el.density.value);
    const zoom = Number(this.el.zoom.value);
    const options = this.options();
    const foreground = this.layerPixels("_foreground.xml", density);
    const background = options.background ? this.layerPixels("_background.xml", density) : null;
    const monochrome = options.monochrome ? this.layerPixels("_monochrome.xml", density) : null;
    if (!foreground) return;

    const size = Math.round(ICON_DP * density);
    const placement = placeIcon(size);
    const foregroundCanvas = pixelCanvas(foreground);
    const backgroundCanvas = background ? pixelCanvas(background) : null;
    const cssSize = (size * zoom) / devicePixelRatio;

    const compose = (maskPath: string, paintBackground: (ctx: CanvasRenderingContext2D) => void, paintForeground: (ctx: CanvasRenderingContext2D) => void) => {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = size;
      canvas.style.width = canvas.style.height = `${cssSize}px`;
      const ctx = canvas.getContext("2d")!;
      const mask = new Path2D();
      mask.addPath(new Path2D(maskPath), new DOMMatrix().scale(placement.maskScale));
      ctx.save();
      ctx.clip(mask);
      paintBackground(ctx);
      paintForeground(ctx);
      ctx.restore();
      if (this.el.safeZone.checked) {
        ctx.strokeStyle = "rgba(16, 26, 22, 0.55)";
        ctx.setLineDash([3 * density, 3 * density]);
        ctx.lineWidth = Math.max(1, density);
        ctx.beginPath();
        ctx.arc(size / 2, size / 2, placement.safeZone / 2, 0, Math.PI * 2);
        ctx.stroke();
      }
      return canvas;
    };
    const drawLayer = (layer: HTMLCanvasElement) => (ctx: CanvasRenderingContext2D) =>
      ctx.drawImage(layer, placement.layerOffset, placement.layerOffset, placement.layerSize, placement.layerSize);
    const paintBackground = backgroundCanvas
      ? drawLayer(backgroundCanvas)
      : (ctx: CanvasRenderingContext2D) => {
          ctx.fillStyle = options.backgroundColor!;
          ctx.fillRect(0, 0, size, size);
        };

    const tiles = MASKS.map((mask) => {
      const canvas = compose(mask.path, paintBackground, drawLayer(foregroundCanvas));
      canvas.setAttribute("role", "img");
      canvas.setAttribute("aria-label", `Icon under the ${mask.label.toLowerCase()} mask`);
      return tile(mask.label, mask.source, canvas);
    });

    if (monochrome) {
      // Android 13 themed icons tint the monochrome layer onto a tonal
      // background; the colors depend on the wallpaper, so these stand in.
      const tinted = pixelCanvas(monochrome);
      const ctx = tinted.getContext("2d")!;
      ctx.globalCompositeOperation = "source-in";
      ctx.fillStyle = "#1f3b2a";
      ctx.fillRect(0, 0, tinted.width, tinted.height);
      const canvas = compose(
        MASKS[1].path,
        (c) => {
          c.fillStyle = "#cfe3cc";
          c.fillRect(0, 0, size, size);
        },
        drawLayer(tinted),
      );
      canvas.setAttribute("role", "img");
      canvas.setAttribute("aria-label", "Themed icon from the monochrome layer");
      tiles.push(tile("Themed", "Android 13+, colors stand in for the wallpaper's", canvas));
    }
    this.el.previews.replaceChildren(...tiles);
  }

  private drawLegacy(): void {
    if (!this.result?.ok) return;
    const options = this.options();
    const legacy = this.result.files.find((f) => /^mipmap(-anydpi-v24)?\/[^/]+\.xml$/.test(f.path) && !f.path.endsWith("_round.xml"));
    this.el.legacyBlock.hidden = !legacy;
    if (!legacy?.xml) return;
    renderColumns(this.el.legacyColumns, this.ck, {
      name: `${options.name} legacy icon`,
      xml: legacy.xml,
      minimumApi: this.result.layers.legacy?.minimum_api ?? null,
      density: Number(this.el.density.value),
      zoom: Number(this.el.zoom.value),
    });
  }

  private async downloadZip(): Promise<void> {
    if (!this.result?.ok) return;
    const entries: { path: string; data: Uint8Array }[] = [];
    const legacyByPath = new Map<string, string>();
    for (const file of this.result.files) {
      if (file.xml) {
        entries.push({ path: `res/${file.path}`, data: this.encoder.encode(file.xml) });
        legacyByPath.set(file.path, file.xml);
      }
    }
    // Legacy PNGs, rendered from the legacy vector as the command does.
    for (const file of this.result.files) {
      if (!file.rendered_from) continue;
      const xml = legacyByPath.get(file.rendered_from);
      const density = LEGACY_DENSITIES.find(([bucket]) => file.path.startsWith(`mipmap-${bucket}/`))?.[1];
      const root = xml ? parseVector(xml).root : null;
      if (!root || !density) continue;
      const rendered = renderVectorDrawable(this.ck, root, { version: "latest", bounds: { width: LEGACY_DP * density, height: LEGACY_DP * density } });
      const blob = await new Promise<Blob | null>((resolve) => pixelCanvas(rendered).toBlob(resolve, "image/png"));
      if (blob) entries.push({ path: `res/${file.path}`, data: new Uint8Array(await blob.arrayBuffer()) });
    }
    const zip = createZip(entries);
    const bytes = new Uint8Array(new ArrayBuffer(zip.length));
    bytes.set(zip);
    downloadBlob(new Blob([bytes], { type: "application/zip" }), `${this.options().name}-res.zip`);
  }
}

function tile(label: string, source: string, canvas: HTMLCanvasElement): HTMLElement {
  const figure = document.createElement("figure");
  figure.className = "tile";
  const caption = document.createElement("figcaption");
  caption.append(
    Object.assign(document.createElement("span"), { className: "tile-label", textContent: label }),
    Object.assign(document.createElement("span"), { className: "tile-source", textContent: source }),
  );
  figure.append(canvas, caption);
  return figure;
}
