// Attribute values as aapt compiles them into a resource. Resource and theme
// references cannot be resolved in a browser, so they are reported instead of
// guessed.

export class DrawableLoadError extends Error {}

const f32 = Math.fround;

export function parseFloatValue(name: string, raw: string): number {
  if (/^\s*[@?]/.test(raw)) throw unsupportedReference(name, raw);
  const trimmed = raw.trim();
  if (!/^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/.test(trimmed)) {
    throw new DrawableLoadError(`${name}="${raw}" is not a number`);
  }
  return f32(Number(trimmed));
}

const DIMENSION_UNITS: Record<string, (value: number, density: number) => number> = {
  px: (value) => value,
  dp: (value, density) => value * density,
  dip: (value, density) => value * density,
  sp: (value, density) => value * density,
  pt: (value, density) => (value * density * 160) / 72,
  in: (value, density) => value * density * 160,
  mm: (value, density) => (value * density * 160) / 25.4,
};

/** A dimension converted to pixels at `density` (1 for mdpi), like `TypedArray.getDimension`. */
export function parseDimension(name: string, raw: string, density: number): number {
  if (/^\s*[@?]/.test(raw)) throw unsupportedReference(name, raw);
  const match = /^\s*([+-]?(?:\d+\.?\d*|\.\d+))\s*([a-z]+)\s*$/.exec(raw);
  const convert = match && DIMENSION_UNITS[match[2]];
  if (!match || !convert) throw new DrawableLoadError(`${name}="${raw}" is not a dimension`);
  return f32(convert(f32(Number(match[1])), density));
}

/** A color as a 32-bit ARGB integer. */
export function parseColor(name: string, raw: string): number {
  if (/^\s*[@?]/.test(raw)) throw unsupportedReference(name, raw);
  const hex = /^\s*#([0-9a-fA-F]+)\s*$/.exec(raw)?.[1] ?? "";
  const digits = hex.length === 3 || hex.length === 4
    ? [...hex].map((digit) => digit + digit).join("")
    : hex;
  if (digits.length === 6) return (0xff000000 | Number.parseInt(digits, 16)) >>> 0;
  if (digits.length === 8) return Number.parseInt(digits, 16) >>> 0;
  throw new DrawableLoadError(`${name}="${raw}" is not a color`);
}

export function parseEnum(name: string, raw: string, values: Record<string, number>): number {
  const value = values[raw.trim()];
  if (value === undefined) {
    if (/^\s*-?\d+\s*$/.test(raw)) return Number(raw);
    throw new DrawableLoadError(`${name}="${raw}" is not one of ${Object.keys(values).join(", ")}`);
  }
  return value;
}

export function parseBoolean(name: string, raw: string): boolean {
  if (raw.trim() === "true") return true;
  if (raw.trim() === "false") return false;
  throw new DrawableLoadError(`${name}="${raw}" is not true or false`);
}

/** `SkColorSetA(color, alpha * SkColorGetA(color))` and the Java equivalent. */
export function applyAlpha(color: number, alpha: number): number {
  const alphaBytes = color >>> 24;
  return ((color & 0x00ffffff) | (Math.trunc(alphaBytes * alpha) << 24)) >>> 0;
}

function unsupportedReference(name: string, raw: string): DrawableLoadError {
  return new DrawableLoadError(
    `${name}="${raw}" refers to a resource or theme attribute, which the preview cannot resolve`,
  );
}
