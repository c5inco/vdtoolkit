// Port of libs/hwui/PathParser.cpp and utils/VectorDrawableUtils.cpp, the
// native android:pathData parser used from API 24. Parsing is unchanged from
// android-7.0.0_r1 to main. Arcs differ: 7.0 converts them to cubic Béziers
// itself, while later versions call SkPath::arcTo.

import { drawArc, type PathSink } from "./path-data-api21.ts";
import { DrawableLoadError } from "./values.ts";

const f32 = Math.fround;
const FLT_MAX = 3.4028234663852886e38;

export interface PathData {
  verbs: string[];
  verbSizes: number[];
  points: number[];
}

export interface HwuiPathSink extends PathSink {
  arcToRotated(rx: number, ry: number, xAxisRotate: number, useSmallArc: boolean,
    isCCW: boolean, x: number, y: number): unknown;
}

export type ArcMode = "bezier" | "skia";

export function getPathDataFromAsciiString(pathStr: string): PathData {
  const data: PathData = { verbs: [], verbSizes: [], points: [] };
  const strLen = pathStr.length;
  let start = 0;
  while (isspace(pathStr.charCodeAt(start)) && start < strLen) start++;
  if (start === strLen) throw failure("Path string cannot be empty.");

  let end = start + 1;
  while (end < strLen) {
    end = nextStart(pathStr, strLen, end);
    const points = getFloats(pathStr, start, end);
    validateVerbAndPoints(pathStr, start, points.length);
    data.verbs.push(pathStr.charAt(start));
    data.verbSizes.push(points.length);
    data.points.push(...points);
    start = end;
    end++;
  }
  if (end - start === 1 && start < strLen) {
    validateVerbAndPoints(pathStr, start, 0);
    data.verbs.push(pathStr.charAt(start));
    data.verbSizes.push(0);
  }
  return data;
}

function failure(message: string): DrawableLoadError {
  return new DrawableLoadError(`${message} (android:pathData)`);
}

function isspace(c: number): boolean {
  return c === 32 || (c >= 9 && c <= 13);
}

function nextStart(s: string, length: number, startIndex: number): number {
  let index = startIndex;
  while (index < length) {
    const c = s.charCodeAt(index);
    if (((c - 65) * (c - 90) <= 0 || (c - 97) * (c - 122) <= 0) && c !== 101 && c !== 69) {
      return index;
    }
    index++;
  }
  return index;
}

function extract(s: string, start: number, end: number): { endPosition: number; endWithNegOrDot: boolean } {
  let currentIndex = start;
  let foundSeparator = false;
  let endWithNegOrDot = false;
  let secondDot = false;
  let isExponential = false;
  for (; currentIndex < end; currentIndex++) {
    const isPrevExponential = isExponential;
    isExponential = false;
    switch (s.charAt(currentIndex)) {
      case " ":
      case ",":
        foundSeparator = true;
        break;
      case "-":
        if (currentIndex !== start && !isPrevExponential) {
          foundSeparator = true;
          endWithNegOrDot = true;
        }
        break;
      case ".":
        if (!secondDot) {
          secondDot = true;
        } else {
          foundSeparator = true;
          endWithNegOrDot = true;
        }
        break;
      case "e":
      case "E":
        isExponential = true;
        break;
    }
    if (foundSeparator) break;
  }
  return { endPosition: currentIndex, endWithNegOrDot };
}

function getFloats(pathStr: string, start: number, end: number): number[] {
  const points: number[] = [];
  const verb = pathStr.charAt(start);
  if (verb === "z" || verb === "Z") return points;
  let startPosition = start + 1;
  while (startPosition < end) {
    const { endPosition, endWithNegOrDot } = extract(pathStr, startPosition, end);
    if (startPosition < endPosition) {
      points.push(parseFloat(pathStr, startPosition, end));
    }
    startPosition = endWithNegOrDot ? endPosition : endPosition + 1;
  }
  return points;
}

// strtof reads the longest number at `startIndex`, even past this segment.
function parseFloat(pathStr: string, startIndex: number, end: number): number {
  const rest = pathStr.substring(startIndex);
  const match = /^[\t\n\v\f\r ]*([+-]?(?:inf(?:inity)?|nan|(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?))/i.exec(rest);
  const snippet = pathStr.substring(startIndex, end);
  if (!match) throw failure(`Float format error when parsing: ${snippet}`);
  const text = match[1].toLowerCase().replace("infinity", "inf");
  const value = text.endsWith("inf") ? (text.startsWith("-") ? -Infinity : Infinity)
    : text.endsWith("nan") ? NaN
    : Number(text);
  if (Number.isFinite(value) && Math.abs(value) > FLT_MAX) {
    throw failure(`Float out of range:  ${snippet}`);
  }
  return f32(value);
}

function validateVerbAndPoints(pathStr: string, start: number, points: number): void {
  const verb = pathStr.charAt(start);
  let expected: number;
  switch (verb) {
    case "z": case "Z":
      expected = 0;
      break;
    case "m": case "l": case "t": case "M": case "L": case "T":
      expected = 2;
      break;
    case "h": case "v": case "H": case "V":
      expected = 1;
      break;
    case "c": case "C":
      expected = 6;
      break;
    case "s": case "q": case "S": case "Q":
      expected = 4;
      break;
    case "a": case "A":
      expected = 7;
      break;
    default:
      throw failure(`${verb} is not a valid verb. Failure occurred at position ${start}`);
  }
  if (expected === 0 && points === 0) return;
  if (expected > 0 && points % expected === 0) return;
  throw failure(
    `${verb} needs to be followed by ${expected > 0 ? "a multiple of " : ""}${expected} floats. ` +
      `However, ${points} float(s) are found. Failure occurred at position ${start}`,
  );
}

export function verbsToPath(outPath: HwuiPathSink, data: PathData, arcMode: ArcMode): void {
  const resolver = new PathResolver(arcMode);
  let previousCommand = "m";
  let start = 0;
  for (let i = 0; i < data.verbs.length; i++) {
    const verbSize = data.verbSizes[i];
    resolver.addCommand(outPath, previousCommand, data.verbs[i], data.points, start, start + verbSize);
    previousCommand = data.verbs[i];
    start += verbSize;
  }
}

class PathResolver {
  currentX = 0;
  currentY = 0;
  ctrlPointX = 0;
  ctrlPointY = 0;
  currentSegmentStartX = 0;
  currentSegmentStartY = 0;

  readonly arcMode: ArcMode;

  constructor(arcMode: ArcMode) {
    this.arcMode = arcMode;
  }

  addCommand(outPath: HwuiPathSink, previousCmd: string, cmd: string, points: number[], start: number, end: number): void {
    let incr = 2;
    let reflectiveCtrlPointX: number;
    let reflectiveCtrlPointY: number;

    switch (cmd) {
      case "z":
      case "Z":
        outPath.close();
        this.currentX = this.currentSegmentStartX;
        this.currentY = this.currentSegmentStartY;
        this.ctrlPointX = this.currentSegmentStartX;
        this.ctrlPointY = this.currentSegmentStartY;
        outPath.moveTo(this.currentX, this.currentY);
        break;
      case "m": case "M": case "l": case "L": case "t": case "T":
        incr = 2;
        break;
      case "h": case "H": case "v": case "V":
        incr = 1;
        break;
      case "c": case "C":
        incr = 6;
        break;
      case "s": case "S": case "q": case "Q":
        incr = 4;
        break;
      case "a": case "A":
        incr = 7;
        break;
    }

    const p = points;
    for (let k = start; k < end; k += incr) {
      switch (cmd) {
        case "m":
          this.currentX = f32(this.currentX + p[k]);
          this.currentY = f32(this.currentY + p[k + 1]);
          if (k > start) {
            outPath.rLineTo(p[k], p[k + 1]);
          } else {
            outPath.rMoveTo(p[k], p[k + 1]);
            this.currentSegmentStartX = this.currentX;
            this.currentSegmentStartY = this.currentY;
          }
          break;
        case "M":
          this.currentX = p[k];
          this.currentY = p[k + 1];
          if (k > start) {
            outPath.lineTo(p[k], p[k + 1]);
          } else {
            outPath.moveTo(p[k], p[k + 1]);
            this.currentSegmentStartX = this.currentX;
            this.currentSegmentStartY = this.currentY;
          }
          break;
        case "l":
          outPath.rLineTo(p[k], p[k + 1]);
          this.currentX = f32(this.currentX + p[k]);
          this.currentY = f32(this.currentY + p[k + 1]);
          break;
        case "L":
          outPath.lineTo(p[k], p[k + 1]);
          this.currentX = p[k];
          this.currentY = p[k + 1];
          break;
        case "h":
          outPath.rLineTo(p[k], 0);
          this.currentX = f32(this.currentX + p[k]);
          break;
        case "H":
          outPath.lineTo(p[k], this.currentY);
          this.currentX = p[k];
          break;
        case "v":
          outPath.rLineTo(0, p[k]);
          this.currentY = f32(this.currentY + p[k]);
          break;
        case "V":
          outPath.lineTo(this.currentX, p[k]);
          this.currentY = p[k];
          break;
        case "c":
          outPath.rCubicTo(p[k], p[k + 1], p[k + 2], p[k + 3], p[k + 4], p[k + 5]);
          this.ctrlPointX = f32(this.currentX + p[k + 2]);
          this.ctrlPointY = f32(this.currentY + p[k + 3]);
          this.currentX = f32(this.currentX + p[k + 4]);
          this.currentY = f32(this.currentY + p[k + 5]);
          break;
        case "C":
          outPath.cubicTo(p[k], p[k + 1], p[k + 2], p[k + 3], p[k + 4], p[k + 5]);
          this.currentX = p[k + 4];
          this.currentY = p[k + 5];
          this.ctrlPointX = p[k + 2];
          this.ctrlPointY = p[k + 3];
          break;
        case "s":
          reflectiveCtrlPointX = 0;
          reflectiveCtrlPointY = 0;
          if ("csCS".includes(previousCmd)) {
            reflectiveCtrlPointX = f32(this.currentX - this.ctrlPointX);
            reflectiveCtrlPointY = f32(this.currentY - this.ctrlPointY);
          }
          outPath.rCubicTo(reflectiveCtrlPointX, reflectiveCtrlPointY, p[k], p[k + 1], p[k + 2], p[k + 3]);
          this.ctrlPointX = f32(this.currentX + p[k]);
          this.ctrlPointY = f32(this.currentY + p[k + 1]);
          this.currentX = f32(this.currentX + p[k + 2]);
          this.currentY = f32(this.currentY + p[k + 3]);
          break;
        case "S":
          reflectiveCtrlPointX = this.currentX;
          reflectiveCtrlPointY = this.currentY;
          if ("csCS".includes(previousCmd)) {
            reflectiveCtrlPointX = f32(2 * this.currentX - this.ctrlPointX);
            reflectiveCtrlPointY = f32(2 * this.currentY - this.ctrlPointY);
          }
          outPath.cubicTo(reflectiveCtrlPointX, reflectiveCtrlPointY, p[k], p[k + 1], p[k + 2], p[k + 3]);
          this.ctrlPointX = p[k];
          this.ctrlPointY = p[k + 1];
          this.currentX = p[k + 2];
          this.currentY = p[k + 3];
          break;
        case "q":
          outPath.rQuadTo(p[k], p[k + 1], p[k + 2], p[k + 3]);
          this.ctrlPointX = f32(this.currentX + p[k]);
          this.ctrlPointY = f32(this.currentY + p[k + 1]);
          this.currentX = f32(this.currentX + p[k + 2]);
          this.currentY = f32(this.currentY + p[k + 3]);
          break;
        case "Q":
          outPath.quadTo(p[k], p[k + 1], p[k + 2], p[k + 3]);
          this.ctrlPointX = p[k];
          this.ctrlPointY = p[k + 1];
          this.currentX = p[k + 2];
          this.currentY = p[k + 3];
          break;
        case "t":
          reflectiveCtrlPointX = 0;
          reflectiveCtrlPointY = 0;
          if ("qtQT".includes(previousCmd)) {
            reflectiveCtrlPointX = f32(this.currentX - this.ctrlPointX);
            reflectiveCtrlPointY = f32(this.currentY - this.ctrlPointY);
          }
          outPath.rQuadTo(reflectiveCtrlPointX, reflectiveCtrlPointY, p[k], p[k + 1]);
          this.ctrlPointX = f32(this.currentX + reflectiveCtrlPointX);
          this.ctrlPointY = f32(this.currentY + reflectiveCtrlPointY);
          this.currentX = f32(this.currentX + p[k]);
          this.currentY = f32(this.currentY + p[k + 1]);
          break;
        case "T":
          reflectiveCtrlPointX = this.currentX;
          reflectiveCtrlPointY = this.currentY;
          if ("qtQT".includes(previousCmd)) {
            reflectiveCtrlPointX = f32(2 * this.currentX - this.ctrlPointX);
            reflectiveCtrlPointY = f32(2 * this.currentY - this.ctrlPointY);
          }
          outPath.quadTo(reflectiveCtrlPointX, reflectiveCtrlPointY, p[k], p[k + 1]);
          this.ctrlPointX = reflectiveCtrlPointX;
          this.ctrlPointY = reflectiveCtrlPointY;
          this.currentX = p[k];
          this.currentY = p[k + 1];
          break;
        case "a":
        case "A": {
          const x = cmd === "a" ? f32(p[k + 5] + this.currentX) : p[k + 5];
          const y = cmd === "a" ? f32(p[k + 6] + this.currentY) : p[k + 6];
          if (this.arcMode === "bezier") {
            drawArc(outPath, this.currentX, this.currentY, x, y, p[k], p[k + 1], p[k + 2],
              p[k + 3] !== 0, p[k + 4] !== 0, nativeSegmentCount);
          } else {
            // SkPath::arcTo(rx, ry, angle, (ArcSize)(large != 0), (SkPathDirection)(sweep == 0), x, y)
            outPath.arcToRotated(p[k], p[k + 1], p[k + 2], !(p[k + 3] !== 0), p[k + 4] === 0, x, y);
          }
          this.currentX = x;
          this.currentY = y;
          this.ctrlPointX = this.currentX;
          this.ctrlPointY = this.currentY;
          break;
        }
      }
      previousCmd = cmd;
    }
  }
}

// C++: ceil(fabs(sweep * 4 / M_PI))
function nativeSegmentCount(sweep: number): number {
  return Math.ceil(Math.abs((sweep * 4) / Math.PI));
}
