// The three-Android comparison: one column per version, a status that says
// whether it agrees with the latest Android, and a loupe shared across all
// three so the same pixels can be compared.

import type { CanvasKit } from "canvaskit-wasm";
import {
  ANDROID_VERSIONS, DrawableLoadError, renderVectorDrawable, type AndroidVersion, type RenderedDrawable,
} from "./android/render.ts";
import type { XmlElement } from "./android/vector-xml.ts";

export const ANDROID_NS = "http://schemas.android.com/apk/res/android";

export interface ColumnsInput {
  /** What is being drawn, for accessibility labels. */
  name: string;
  xml: string | null;
  /** Why there is no XML, when there is none. */
  error?: string | null;
  /** The minimum API vdt reported for the source, if known. */
  minimumApi?: number | null;
  /** Why a version disagrees, when the sample is known. */
  reasons?: Partial<Record<AndroidVersion, string>>;
  density: number;
  zoom: number;
}

interface View {
  result: RenderedDrawable | Error;
  canvas: HTMLCanvasElement | null;
  loupe: HTMLCanvasElement | null;
  caption: HTMLElement | null;
  stage: HTMLElement;
}

const viewsByContainer = new WeakMap<HTMLElement, View[]>();

export function parseVector(xml: string): { root: XmlElement | null; error: string | null; viewportWidth: number } {
  const parsed = new DOMParser().parseFromString(xml, "application/xml");
  const error = parsed.querySelector("parsererror") ? "the XML is not well formed" : null;
  const viewportWidth = Number(parsed.documentElement.getAttributeNS(ANDROID_NS, "viewportWidth")) || 0;
  return { root: error ? null : (parsed.documentElement as unknown as XmlElement), error, viewportWidth };
}

export function renderColumns(container: HTMLElement, ck: CanvasKit, input: ColumnsInput): void {
  container.replaceChildren();
  const views: View[] = [];
  viewsByContainer.set(container, views);

  const parsed = input.xml === null ? null : parseVector(input.xml);
  const results = new Map<AndroidVersion, RenderedDrawable | Error>();
  for (const { id } of ANDROID_VERSIONS) {
    if (!parsed?.root) {
      results.set(id, new DrawableLoadError(parsed?.error ?? input.error ?? "nothing to draw"));
      continue;
    }
    try {
      results.set(id, renderVectorDrawable(ck, parsed.root, { version: id, density: input.density }));
    } catch (error) {
      results.set(id, error instanceof Error ? error : new Error(String(error)));
    }
  }
  const latest = results.get("latest")!;
  const minimumApi = input.minimumApi ?? null;

  for (const version of ANDROID_VERSIONS) {
    const result = results.get(version.id)!;
    const column = document.createElement("div");
    column.className = "column";

    const status = Object.assign(document.createElement("span"), { className: "status" });
    if (version.id === "latest") {
      column.dataset.state = "reference";
      status.textContent = "reference";
    } else if (result instanceof Error) {
      column.dataset.state = "error";
      status.textContent = "fails to load";
    } else if (latest instanceof Error || !differs(result, latest)) {
      column.dataset.state = "same";
      status.textContent = "same as latest";
    } else {
      column.dataset.state = "differs";
      status.textContent = "differs";
    }

    const head = document.createElement("div");
    head.className = "column-head";
    head.append(
      Object.assign(document.createElement("span"), { className: "title", textContent: version.label }),
      Object.assign(document.createElement("span"), { className: "source", textContent: version.source }),
      status,
    );

    const stage = document.createElement("div");
    stage.className = "stage";
    const view: View = { result, canvas: null, loupe: null, caption: null, stage };
    const foot = document.createElement("div");
    foot.className = "meta";

    if (result instanceof Error) {
      stage.append(Object.assign(document.createElement("div"), { className: "empty", textContent: "Not drawn" }));
    } else {
      const frame = document.createElement("div");
      frame.className = "frame";
      const cssWidth = (result.width * input.zoom) / devicePixelRatio;
      const viewportWidth = parsed?.viewportWidth ?? 0;
      frame.style.setProperty("--unit", `${viewportWidth > 0 ? cssWidth / viewportWidth : cssWidth}px`);

      const canvas = pixelCanvas(result);
      canvas.style.width = `${cssWidth}px`;
      canvas.style.height = `${(result.height * input.zoom) / devicePixelRatio}px`;
      canvas.setAttribute("role", "img");
      canvas.setAttribute("aria-label", `${input.name} drawn by ${version.label}`);

      const loupe = document.createElement("div");
      loupe.className = "loupe";
      const loupeCanvas = document.createElement("canvas");
      loupeCanvas.width = loupeCanvas.height = LOUPE_CELLS * LOUPE_SCALE * devicePixelRatio;
      loupeCanvas.style.width = loupeCanvas.style.height = `${LOUPE_CELLS * LOUPE_SCALE}px`;
      const caption = Object.assign(document.createElement("div"), { className: "loupe-caption" });
      loupe.append(loupeCanvas, caption);

      frame.append(canvas, loupe);
      stage.append(frame);
      view.canvas = canvas;
      view.loupe = loupeCanvas;
      view.caption = caption;
      frame.addEventListener("pointermove", (event) => inspect(views, view, event));
      frame.addEventListener("pointerleave", () => inspect(views, null));

      foot.append(Object.assign(document.createElement("span"), { textContent: `${result.width} × ${result.height} px` }));
    }

    column.append(head, stage, foot);

    if (result instanceof Error) {
      column.append(note("error", result instanceof DrawableLoadError
        ? `Fails to load: ${result.message}.`
        : `The preview failed: ${result.message}`));
    } else if (column.dataset.state === "differs") {
      column.append(note("differs", input.reasons?.[version.id] ?? "Draws differently from the latest Android."));
    }
    if (version.id === "api21" && minimumApi !== null && minimumApi > 21) {
      column.append(note("never",
        `vdt says this needs API ${minimumApi}. Ship it in drawable-v${minimumApi} with a fallback, and ` +
        `API 21 to ${minimumApi - 1} never load it${result instanceof Error ? "" : "; this is what they would draw if they did"}.`));
    }
    views.push(view);
    container.append(column);
  }
}

/** A canvas holding the rendered pixels one to one. */
export function pixelCanvas(result: RenderedDrawable): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = result.width;
  canvas.height = result.height;
  canvas.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(result.pixels), result.width, result.height), 0, 0);
  return canvas;
}

export function note(kind: string, text: string): HTMLParagraphElement {
  return Object.assign(document.createElement("p"), { className: `note ${kind}`, textContent: text });
}

const LOUPE_CELLS = 11;
const LOUPE_SCALE = 12;

function inspect(views: View[], source: View | null, event?: PointerEvent): void {
  if (!source || !event || !source.canvas || source.result instanceof Error) {
    for (const view of views) view.stage.classList.remove("inspecting");
    return;
  }
  const rect = source.canvas.getBoundingClientRect();
  const x = Math.floor(((event.clientX - rect.left) / rect.width) * source.result.width);
  const y = Math.floor(((event.clientY - rect.top) / rect.height) * source.result.height);

  for (const view of views) {
    if (!view.canvas || !view.loupe || !view.caption || view.result instanceof Error) continue;
    view.stage.classList.add("inspecting");
    const ctx = view.loupe.getContext("2d")!;
    const size = view.loupe.width;
    const cell = size / LOUPE_CELLS;
    const half = Math.floor(LOUPE_CELLS / 2);
    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = "#e9ece7";
    ctx.fillRect(0, 0, size, size);
    ctx.drawImage(view.canvas, x - half, y - half, LOUPE_CELLS, LOUPE_CELLS, 0, 0, size, size);
    ctx.strokeStyle = "rgba(16, 26, 22, 0.12)";
    ctx.lineWidth = 1;
    for (let i = 1; i < LOUPE_CELLS; i++) {
      ctx.beginPath();
      ctx.moveTo(i * cell, 0);
      ctx.lineTo(i * cell, size);
      ctx.moveTo(0, i * cell);
      ctx.lineTo(size, i * cell);
      ctx.stroke();
    }
    ctx.strokeStyle = "#101a16";
    ctx.lineWidth = 2 * devicePixelRatio;
    ctx.strokeRect(half * cell + 1, half * cell + 1, cell - 2, cell - 2);

    const { width, height, pixels } = view.result;
    const inside = x >= 0 && y >= 0 && x < width && y < height;
    const i = (y * width + x) * 4;
    const hex = inside
      ? "#" + [pixels[i + 3], pixels[i], pixels[i + 1], pixels[i + 2]].map((v) => v.toString(16).padStart(2, "0")).join("")
      : "";
    view.caption.textContent = `${x}, ${y}  ${hex}`;
  }
}

// Ignores edge smoothing: counts pixels whose color or opacity moved by more than a quarter.
function differs(a: RenderedDrawable, b: RenderedDrawable): boolean {
  if (a.width !== b.width || a.height !== b.height) return true;
  let changed = 0;
  for (let i = 0; i < a.pixels.length; i += 4) {
    const alpha = Math.min(a.pixels[i + 3], b.pixels[i + 3]) / 255;
    const delta = Math.max(
      Math.abs(a.pixels[i + 3] - b.pixels[i + 3]),
      Math.abs(a.pixels[i] - b.pixels[i]) * alpha,
      Math.abs(a.pixels[i + 1] - b.pixels[i + 1]) * alpha,
      Math.abs(a.pixels[i + 2] - b.pixels[i + 2]) * alpha,
    );
    if (delta > 64) changed++;
  }
  return changed > (a.width * a.height) / 200;
}
