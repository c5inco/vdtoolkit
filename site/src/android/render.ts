// Renders VectorDrawable XML the way a given Android version draws it.

import type { CanvasKit } from "canvaskit-wasm";
import { Scope } from "./canvas.ts";
import { drawApi21, inflateApi21 } from "./render-api21.ts";
import { drawApi24, inflateHwui } from "./render-api24.ts";
import { drawLatest } from "./render-latest.ts";
import { DrawableLoadError, parseDimension } from "./values.ts";
import { readVectorXml, type VectorDocument, type XmlElement } from "./vector-xml.ts";

export { DrawableLoadError } from "./values.ts";

export type AndroidVersion = "api21" | "api24" | "latest";

export const ANDROID_VERSIONS: { id: AndroidVersion; label: string; source: string }[] = [
  { id: "api21", label: "Android 5", source: "Lollipop · API 21" },
  { id: "api24", label: "Android 7", source: "Nougat · API 24" },
  { id: "latest", label: "Latest", source: "AOSP main" },
];

export interface RenderOptions {
  version: AndroidVersion;
  /** Screen density, where 1 is mdpi and 3 is xxhdpi. Sets the intrinsic size. */
  density?: number;
  /** Drawable bounds in pixels, as `setBounds(0, 0, width, height)`; defaults to the intrinsic size. */
  bounds?: { width: number; height: number };
}

export interface RenderedDrawable {
  width: number;
  height: number;
  /** Unpremultiplied RGBA, like `Bitmap.getPixel`. */
  pixels: Uint8Array;
}

/**
 * Inflates and draws `root`, a parsed <vector> element. Throws
 * DrawableLoadError where that Android version would fail to load the file.
 */
export function renderVectorDrawable(ck: CanvasKit, root: XmlElement, options: RenderOptions): RenderedDrawable {
  const document = readVectorXml(root);
  const bounds = options.bounds ?? intrinsicSize(document, options.version, options.density ?? 1);
  const { width, height } = bounds;
  if (width <= 0 || height <= 0) throw new DrawableLoadError("the drawable has no size to draw at");

  const scope = new Scope();
  try {
    // Every version draws into an offscreen bitmap the size of the bounds
    // (when the canvas is not scaled), then copies it with the root alpha and tint.
    const cache = scope.keep(ck.MakeSurface(width, height) ?? fail("could not allocate a surface"));
    const cacheCanvas = cache.getCanvas();
    cacheCanvas.clear(ck.TRANSPARENT);
    switch (options.version) {
      case "api21":
        drawApi21(ck, scope, cacheCanvas, inflateApi21(document), width, height);
        break;
      case "api24":
        drawApi24(ck, scope, cacheCanvas, inflateHwui(document), width, height);
        break;
      case "latest":
        drawLatest(ck, scope, cacheCanvas, inflateHwui(document), width, height);
        break;
    }

    const output = scope.keep(ck.MakeSurface(width, height) ?? fail("could not allocate a surface"));
    const canvas = output.getCanvas();
    canvas.clear(ck.TRANSPARENT);
    const paint = scope.keep(new ck.Paint());
    paint.setAlphaf(Math.trunc(document.alpha * 255) / 255);
    if (document.tint !== null) {
      const tint = document.tint;
      paint.setColorFilter(scope.keep(ck.ColorFilter.MakeBlend(
        ck.Color((tint >>> 16) & 0xff, (tint >>> 8) & 0xff, tint & 0xff, (tint >>> 24) / 255),
        tintBlendMode(ck, document.tintMode),
      )));
    }
    canvas.drawImage(scope.keep(cache.makeImageSnapshot()), 0, 0, paint);

    const pixels = canvas.readPixels(0, 0, {
      width,
      height,
      colorType: ck.ColorType.RGBA_8888,
      alphaType: ck.AlphaType.Unpremul,
      colorSpace: ck.ColorSpace.SRGB,
    });
    if (!pixels) fail("could not read the rendered pixels");
    return { width, height, pixels: new Uint8Array(pixels as Uint8Array) };
  } finally {
    scope.dispose();
  }
}

/**
 * The size an ImageView gives the drawable. API 21 to 23 truncate
 * `getDimension`; later versions round with `getDimensionPixelSize`.
 */
export function intrinsicSize(document: VectorDocument, version: AndroidVersion, density: number) {
  const size = (name: string, raw: string) => {
    const pixels = parseDimension(`android:${name}`, raw, density);
    if (version !== "latest") return Math.trunc(pixels);
    const rounded = Math.trunc(pixels >= 0 ? Math.fround(pixels + 0.5) : Math.fround(pixels - 0.5));
    return rounded !== 0 || pixels === 0 ? rounded : pixels > 0 ? 1 : -1;
  };
  return { width: size("width", document.width), height: size("height", document.height) };
}

function tintBlendMode(ck: CanvasKit, mode: number) {
  switch (mode) {
    case 3: return ck.BlendMode.SrcOver;
    case 9: return ck.BlendMode.SrcATop;
    case 14: return ck.BlendMode.Modulate;
    case 15: return ck.BlendMode.Screen;
    case 16: return ck.BlendMode.Plus;
    default: return ck.BlendMode.SrcIn;
  }
}

function fail(message: string): never {
  throw new Error(message);
}
