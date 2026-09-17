// Reads VectorDrawable XML into the attributes the framework inflaters see.
// Values are left as the XML wrote them where Android versions disagree about
// them (path data, dimensions); each renderer interprets those itself.

import { DrawableLoadError, parseBoolean, parseColor, parseEnum, parseFloatValue } from "./values.ts";

export const ANDROID_NS = "http://schemas.android.com/apk/res/android";
export const AAPT_NS = "http://schemas.android.com/aapt";

/** The subset of DOM `Element` used here, satisfied by browsers and xmldom. */
export interface XmlElement {
  readonly nodeType: number;
  readonly localName: string | null;
  readonly namespaceURI: string | null;
  readonly childNodes: ArrayLike<XmlElement>;
  getAttributeNS(namespace: string | null, localName: string): string | null;
  hasAttributeNS(namespace: string | null, localName: string): boolean;
  getAttribute(name: string): string | null;
}

export interface Gradient {
  type: number; // 0 linear, 1 radial, 2 sweep
  startX: number;
  startY: number;
  endX: number;
  endY: number;
  centerX: number;
  centerY: number;
  gradientRadius: number;
  tileMode: number; // 0 clamp, 1 repeat, 2 mirror
  /** Colors and offsets as `GradientColor.onColorsChange` resolves them. */
  colors: number[];
  offsets: number[] | null;
}

export type Paint = { kind: "color"; argb: number } | { kind: "gradient"; gradient: Gradient };

export interface VectorGroup {
  kind: "group";
  rotation: number;
  pivotX: number;
  pivotY: number;
  scaleX: number;
  scaleY: number;
  translateX: number;
  translateY: number;
  children: VectorNode[];
}

export interface VectorClipPath {
  kind: "clip-path";
  pathData: string | null;
}

export interface VectorPath {
  kind: "path";
  pathData: string | null;
  fill: Paint | null;
  stroke: Paint | null;
  strokeWidth: number;
  strokeAlpha: number;
  fillAlpha: number;
  trimPathStart: number;
  trimPathEnd: number;
  trimPathOffset: number;
  strokeLineCap: number; // 0 butt, 1 round, 2 square
  strokeLineJoin: number; // 0 miter, 1 round, 2 bevel
  strokeMiterLimit: number;
  /** Only API 24 and later read this attribute. */
  fillType: number; // 0 nonZero, 1 evenOdd
}

export type VectorNode = VectorGroup | VectorClipPath | VectorPath;

export interface VectorDocument {
  width: string;
  height: string;
  viewportWidth: number;
  viewportHeight: number;
  alpha: number;
  tint: number | null;
  tintMode: number; // PorterDuff mode ids from attrs.xml; 5 is src_in
  root: VectorGroup;
}

export function readVectorXml(root: XmlElement): VectorDocument {
  if (root.localName !== "vector") {
    throw new DrawableLoadError(`expected a <vector> root element, found <${root.localName}>`);
  }
  const width = root.getAttributeNS(ANDROID_NS, "width");
  const height = root.getAttributeNS(ANDROID_NS, "height");
  if (width === null) throw new DrawableLoadError("<vector> tag requires width > 0");
  if (height === null) throw new DrawableLoadError("<vector> tag requires height > 0");

  const viewportWidth = float(root, "viewportWidth", 0);
  const viewportHeight = float(root, "viewportHeight", 0);
  if (viewportWidth <= 0) throw new DrawableLoadError("<vector> tag requires viewportWidth > 0");
  if (viewportHeight <= 0) throw new DrawableLoadError("<vector> tag requires viewportHeight > 0");

  const tint = attribute(root, "tint");
  const document: VectorDocument = {
    width,
    height,
    viewportWidth,
    viewportHeight,
    alpha: float(root, "alpha", 1),
    tint: tint === null ? null : parseColor("android:tint", tint),
    tintMode: enumeration(root, "tintMode", TINT_MODES, 5),
    root: emptyGroup(),
  };
  if (attribute(root, "autoMirrored") !== null) {
    parseBoolean("android:autoMirrored", attribute(root, "autoMirrored")!);
  }

  // The framework walks every start tag below <vector> with a pull parser, so
  // a <path> inside an unknown element still joins the current group.
  const groups = [document.root];
  let hasPath = false;
  const visit = (element: XmlElement) => {
    for (const child of elements(element)) {
      if (child.namespaceURI === AAPT_NS) continue;
      const group = groups[groups.length - 1];
      switch (child.localName) {
        case "path":
          group.children.push(readPath(child));
          hasPath = true;
          break;
        case "clip-path":
          group.children.push({ kind: "clip-path", pathData: attribute(child, "pathData") });
          break;
        case "group": {
          const nested = readGroup(child);
          group.children.push(nested);
          groups.push(nested);
          visit(child);
          groups.pop();
          continue;
        }
      }
      visit(child);
    }
  };
  visit(root);
  if (!hasPath) throw new DrawableLoadError("no path defined");
  return document;
}

const TINT_MODES = { src_over: 3, src_in: 5, src_atop: 9, multiply: 14, screen: 15, add: 16 };

function emptyGroup(): VectorGroup {
  return {
    kind: "group",
    rotation: 0,
    pivotX: 0,
    pivotY: 0,
    scaleX: 1,
    scaleY: 1,
    translateX: 0,
    translateY: 0,
    children: [],
  };
}

function readGroup(element: XmlElement): VectorGroup {
  return {
    kind: "group",
    rotation: float(element, "rotation", 0),
    pivotX: float(element, "pivotX", 0),
    pivotY: float(element, "pivotY", 0),
    scaleX: float(element, "scaleX", 1),
    scaleY: float(element, "scaleY", 1),
    translateX: float(element, "translateX", 0),
    translateY: float(element, "translateY", 0),
    children: [],
  };
}

function readPath(element: XmlElement): VectorPath {
  return {
    kind: "path",
    pathData: attribute(element, "pathData"),
    fill: paint(element, "fillColor"),
    stroke: paint(element, "strokeColor"),
    strokeWidth: float(element, "strokeWidth", 0),
    strokeAlpha: float(element, "strokeAlpha", 1),
    fillAlpha: float(element, "fillAlpha", 1),
    trimPathStart: float(element, "trimPathStart", 0),
    trimPathEnd: float(element, "trimPathEnd", 1),
    trimPathOffset: float(element, "trimPathOffset", 0),
    strokeLineCap: enumeration(element, "strokeLineCap", { butt: 0, round: 1, square: 2 }, 0),
    strokeLineJoin: enumeration(element, "strokeLineJoin", { miter: 0, round: 1, bevel: 2 }, 0),
    strokeMiterLimit: float(element, "strokeMiterLimit", 4),
    fillType: enumeration(element, "fillType", { nonZero: 0, evenOdd: 1 }, 0),
  };
}

function paint(element: XmlElement, name: string): Paint | null {
  const inline = elements(element).find(
    (child) =>
      child.namespaceURI === AAPT_NS &&
      child.localName === "attr" &&
      child.getAttribute("name") === `android:${name}`,
  );
  if (inline) {
    const gradient = elements(inline).find((child) => child.localName === "gradient");
    if (!gradient) {
      throw new DrawableLoadError(`<aapt:attr name="android:${name}"> must contain a <gradient>`);
    }
    return { kind: "gradient", gradient: readGradient(gradient) };
  }
  const raw = attribute(element, name);
  return raw === null ? null : { kind: "color", argb: parseColor(`android:${name}`, raw) };
}

// GradientColor.updateRootElementState, inflateChildElements, and onColorsChange.
function readGradient(element: XmlElement): Gradient {
  const type = enumeration(element, "type", { linear: 0, radial: 1, sweep: 2 }, 0);
  const gradientRadius = float(element, "gradientRadius", 0);
  if (gradientRadius <= 0 && type === 1) {
    throw new DrawableLoadError("<gradient> tag requires 'gradientRadius' attribute with radial type");
  }

  const items = elements(element).filter((child) => child.localName === "item");
  let colors: number[];
  let offsets: number[] | null;
  if (items.length > 0) {
    colors = [];
    offsets = [];
    for (const item of items) {
      const color = attribute(item, "color");
      const offset = attribute(item, "offset");
      if (color === null || offset === null) {
        throw new DrawableLoadError("<item> tag requires a 'color' attribute and a 'offset' attribute!");
      }
      colors.push(parseColor("android:color", color));
      offsets.push(parseFloatValue("android:offset", offset));
    }
  } else {
    const startColor = color(element, "startColor");
    const endColor = color(element, "endColor");
    if (attribute(element, "centerColor") !== null) {
      colors = [startColor, color(element, "centerColor"), endColor];
      offsets = [0, 0.5, 1];
    } else {
      colors = [startColor, endColor];
      offsets = null;
    }
  }

  return {
    type,
    startX: float(element, "startX", 0),
    startY: float(element, "startY", 0),
    endX: float(element, "endX", 0),
    endY: float(element, "endY", 0),
    centerX: float(element, "centerX", 0),
    centerY: float(element, "centerY", 0),
    gradientRadius,
    tileMode: enumeration(element, "tileMode", { disabled: -1, clamp: 0, repeat: 1, mirror: 2 }, 0),
    colors,
    offsets,
  };
}

function elements(element: XmlElement): XmlElement[] {
  return Array.from(element.childNodes).filter((child) => child.nodeType === 1);
}

function attribute(element: XmlElement, name: string): string | null {
  return element.hasAttributeNS(ANDROID_NS, name) ? element.getAttributeNS(ANDROID_NS, name) : null;
}

function float(element: XmlElement, name: string, fallback: number): number {
  const raw = attribute(element, name);
  return raw === null ? fallback : parseFloatValue(`android:${name}`, raw);
}

function color(element: XmlElement, name: string): number {
  const raw = attribute(element, name);
  return raw === null ? 0 : parseColor(`android:${name}`, raw);
}

function enumeration(
  element: XmlElement,
  name: string,
  values: Record<string, number>,
  fallback: number,
): number {
  const raw = attribute(element, name);
  return raw === null ? fallback : parseEnum(`android:${name}`, raw, values);
}
