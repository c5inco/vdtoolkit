// The Android comparison: one column per version worth shipping to, and a
// status that says whether it agrees with the latest Android — and by how much.

import type { CanvasKit } from "canvaskit-wasm";
import {
  ANDROID_VERSIONS, DrawableLoadError, renderVectorDrawable, type AndroidVersion, type RenderedDrawable,
} from "./android/render.ts";
import type { XmlElement } from "./android/vector-xml.ts";

export const ANDROID_NS = "http://schemas.android.com/apk/res/android";

/** The page compares the oldest Android worth supporting against the current
 *  one. API 21 to 23 fall below AndroidX's minSdk of 23 and below anything the
 *  converter's "needs API 24" diagnostics would ship to, so the API 21 port
 *  stays in src/android and its tests but off the page. */
const SHOWN: AndroidVersion[] = ["api24", "latest"];

export interface ColumnsInput {
  /** What is being drawn, for accessibility labels. */
  name: string;
  xml: string | null;
  /** Why there is no XML, when there is none. */
  error?: string | null;
  /** Why a version disagrees, when the sample is known. */
  reasons?: Partial<Record<AndroidVersion, string>>;
  density: number;
  zoom: number;
}

export function parseVector(xml: string): { root: XmlElement | null; error: string | null; viewportWidth: number } {
  const parsed = new DOMParser().parseFromString(xml, "application/xml");
  const error = parsed.querySelector("parsererror") ? "the XML is not well formed" : null;
  const viewportWidth = Number(parsed.documentElement.getAttributeNS(ANDROID_NS, "viewportWidth")) || 0;
  return { root: error ? null : (parsed.documentElement as unknown as XmlElement), error, viewportWidth };
}

export function renderColumns(container: HTMLElement, ck: CanvasKit, input: ColumnsInput): void {
  container.replaceChildren();

  const parsed = input.xml === null ? null : parseVector(input.xml);
  const results = new Map<AndroidVersion, RenderedDrawable | Error>();
  for (const id of SHOWN) {
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

  for (const version of ANDROID_VERSIONS.filter(({ id }) => SHOWN.includes(id))) {
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
    } else {
      const share = latest instanceof Error ? 0 : diffShare(result, latest);
      if (latest instanceof Error || share <= 0.005) {
        column.dataset.state = "same";
        status.textContent = "same as latest";
      } else {
        column.dataset.state = "differs";
        status.textContent = `${formatPercent(share)} of pixels differ`;
      }
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

      frame.append(canvas);
      stage.append(frame);

      const dpW = trimDp(result.width / input.density);
      const dpH = trimDp(result.height / input.density);
      const dp = dpW === dpH ? `${dpW} dp` : `${dpW} × ${dpH} dp`;
      foot.append(Object.assign(document.createElement("span"), {
        textContent: `${result.width} × ${result.height} px · ${dp}`,
      }));
    }

    column.append(head, stage, foot);

    if (result instanceof Error) {
      column.append(note("error", result instanceof DrawableLoadError
        ? `Fails to load: ${result.message}.`
        : `The preview failed: ${result.message}`));
    } else if (column.dataset.state === "differs") {
      column.append(note("differs", input.reasons?.[version.id] ?? "Draws differently from the latest Android."));
    }
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

function trimDp(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function formatPercent(fraction: number): string {
  const percent = fraction * 100;
  return percent < 10 ? `${percent.toFixed(1)}%` : `${Math.round(percent)}%`;
}

/**
 * Share of pixels that disagree, measured as color distance so a difference
 * spread across channels — the way unpremultiplied gradients shift a hue —
 * counts as much as a single-channel jump. Where the two versions cover a
 * pixel differently — an antialiased edge — the color difference is ignored
 * as smoothing. One means the two cannot be compared pixel by pixel.
 */
function diffShare(a: RenderedDrawable, b: RenderedDrawable): number {
  if (a.width !== b.width || a.height !== b.height) return 1;
  let changed = 0;
  for (let i = 0; i < a.pixels.length; i += 4) {
    const agree = 1 - Math.abs(a.pixels[i + 3] - b.pixels[i + 3]) / 255;
    const alpha = a.pixels[i + 3] - b.pixels[i + 3];
    const red = (a.pixels[i] - b.pixels[i]) * agree;
    const green = (a.pixels[i + 1] - b.pixels[i + 1]) * agree;
    const blue = (a.pixels[i + 2] - b.pixels[i + 2]) * agree;
    if (Math.sqrt(alpha * alpha + red * red + green * green + blue * blue) > 48) changed++;
  }
  return changed / (a.width * a.height);
}
