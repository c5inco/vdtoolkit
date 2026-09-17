// Port of android.util.PathParser from Android 5.0 (android-5.0.0_r1), which
// API 21 uses to read android:pathData. Control flow follows the Java source
// so its quirks carry over: numbers split only at spaces, commas, and minus
// signs, so `1.5.5` and `1e-5` fail to parse, and `z` does not move the
// current point back to the start of the subpath.

import { DrawableLoadError } from "./values.ts";

const f32 = Math.fround;

export interface PathSink {
  moveTo(x: number, y: number): unknown;
  rMoveTo(x: number, y: number): unknown;
  lineTo(x: number, y: number): unknown;
  rLineTo(x: number, y: number): unknown;
  cubicTo(x1: number, y1: number, x2: number, y2: number, x3: number, y3: number): unknown;
  rCubicTo(x1: number, y1: number, x2: number, y2: number, x3: number, y3: number): unknown;
  quadTo(x1: number, y1: number, x2: number, y2: number): unknown;
  rQuadTo(x1: number, y1: number, x2: number, y2: number): unknown;
  close(): unknown;
}

export interface PathDataNode {
  type: string;
  params: number[];
}

export function createNodesFromPathData(pathData: string): PathDataNode[] {
  let start = 0;
  let end = 1;
  const list: PathDataNode[] = [];
  while (end < pathData.length) {
    end = nextStart(pathData, end);
    const s = javaTrim(pathData.substring(start, end));
    if (s.length > 0) {
      list.push({ type: s.charAt(0), params: getFloats(s) });
    }
    start = end;
    end++;
  }
  if (end - start === 1 && start < pathData.length) {
    list.push({ type: pathData.charAt(start), params: [] });
  }
  return list;
}

function nextStart(s: string, end: number): number {
  while (end < s.length) {
    const c = s.charCodeAt(end);
    if ((c - 65) * (c - 90) <= 0 || (c - 97) * (c - 122) <= 0) return end;
    end++;
  }
  return end;
}

function getFloats(s: string): number[] {
  if (s.charAt(0) === "z" || s.charAt(0) === "Z") return [];
  const results: number[] = [];
  let startPosition = 1;
  const totalLength = s.length;
  while (startPosition < totalLength) {
    const { endPosition, endWithNegSign } = extract(s, startPosition);
    if (startPosition < endPosition) {
      results.push(javaParseFloat(s.substring(startPosition, endPosition)));
    }
    startPosition = endWithNegSign ? endPosition : endPosition + 1;
  }
  return results;
}

function extract(s: string, start: number): { endPosition: number; endWithNegSign: boolean } {
  let currentIndex = start;
  let foundSeparator = false;
  let endWithNegSign = false;
  for (; currentIndex < s.length; currentIndex++) {
    switch (s.charAt(currentIndex)) {
      case " ":
      case ",":
        foundSeparator = true;
        break;
      case "-":
        if (currentIndex !== start) {
          foundSeparator = true;
          endWithNegSign = true;
        }
        break;
    }
    if (foundSeparator) break;
  }
  return { endPosition: currentIndex, endWithNegSign };
}

// Float.parseFloat: surrounding whitespace is ignored, and a type suffix is allowed.
function javaParseFloat(text: string): number {
  const trimmed = javaTrim(text);
  const match = /^([+-]?)(NaN|Infinity|(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)[fFdD]?$/.exec(trimmed);
  if (!match) {
    throw new DrawableLoadError(`Android 5.0 cannot read the number "${text}" in android:pathData`);
  }
  return f32(Number(match[1] + match[2]));
}

function javaTrim(s: string): string {
  let start = 0;
  let end = s.length;
  while (start < end && s.charCodeAt(start) <= 32) start++;
  while (end > start && s.charCodeAt(end - 1) <= 32) end--;
  return s.substring(start, end);
}

export function nodesToPath(nodes: PathDataNode[], path: PathSink): void {
  const current = [0, 0, 0, 0];
  let previousCommand = "m";
  for (const node of nodes) {
    addCommand(path, current, previousCommand, node.type, node.params);
    previousCommand = node.type;
  }
}

function addCommand(path: PathSink, current: number[], previousCmd: string, cmd: string, val: number[]): void {
  let incr = 2;
  let [currentX, currentY, ctrlPointX, ctrlPointY] = current;
  let reflectiveCtrlPointX: number;
  let reflectiveCtrlPointY: number;

  switch (cmd) {
    case "z":
    case "Z":
      path.close();
      return;
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

  // Java throws ArrayIndexOutOfBoundsException when a command is short of
  // numbers; a missing float reads as undefined here, so check explicitly.
  const at = (index: number): number => {
    if (index >= val.length) {
      throw new DrawableLoadError(`Android 5.0 crashes on "${cmd}" without enough numbers in android:pathData`);
    }
    return val[index];
  };

  for (let k = 0; k < val.length; k += incr) {
    switch (cmd) {
      case "m":
        path.rMoveTo(at(k), at(k + 1));
        currentX = f32(currentX + at(k));
        currentY = f32(currentY + at(k + 1));
        break;
      case "M":
        path.moveTo(at(k), at(k + 1));
        currentX = at(k);
        currentY = at(k + 1);
        break;
      case "l":
        path.rLineTo(at(k), at(k + 1));
        currentX = f32(currentX + at(k));
        currentY = f32(currentY + at(k + 1));
        break;
      case "L":
        path.lineTo(at(k), at(k + 1));
        currentX = at(k);
        currentY = at(k + 1);
        break;
      case "h":
        path.rLineTo(at(k), 0);
        currentX = f32(currentX + at(k));
        break;
      case "H":
        path.lineTo(at(k), currentY);
        currentX = at(k);
        break;
      case "v":
        path.rLineTo(0, at(k));
        currentY = f32(currentY + at(k));
        break;
      case "V":
        path.lineTo(currentX, at(k));
        currentY = at(k);
        break;
      case "c":
        path.rCubicTo(at(k), at(k + 1), at(k + 2), at(k + 3), at(k + 4), at(k + 5));
        ctrlPointX = f32(currentX + at(k + 2));
        ctrlPointY = f32(currentY + at(k + 3));
        currentX = f32(currentX + at(k + 4));
        currentY = f32(currentY + at(k + 5));
        break;
      case "C":
        path.cubicTo(at(k), at(k + 1), at(k + 2), at(k + 3), at(k + 4), at(k + 5));
        currentX = at(k + 4);
        currentY = at(k + 5);
        ctrlPointX = at(k + 2);
        ctrlPointY = at(k + 3);
        break;
      case "s":
        reflectiveCtrlPointX = 0;
        reflectiveCtrlPointY = 0;
        if ("csCS".includes(previousCmd)) {
          reflectiveCtrlPointX = f32(currentX - ctrlPointX);
          reflectiveCtrlPointY = f32(currentY - ctrlPointY);
        }
        path.rCubicTo(reflectiveCtrlPointX, reflectiveCtrlPointY, at(k), at(k + 1), at(k + 2), at(k + 3));
        ctrlPointX = f32(currentX + at(k));
        ctrlPointY = f32(currentY + at(k + 1));
        currentX = f32(currentX + at(k + 2));
        currentY = f32(currentY + at(k + 3));
        break;
      case "S":
        reflectiveCtrlPointX = currentX;
        reflectiveCtrlPointY = currentY;
        if ("csCS".includes(previousCmd)) {
          reflectiveCtrlPointX = f32(2 * currentX - ctrlPointX);
          reflectiveCtrlPointY = f32(2 * currentY - ctrlPointY);
        }
        path.cubicTo(reflectiveCtrlPointX, reflectiveCtrlPointY, at(k), at(k + 1), at(k + 2), at(k + 3));
        ctrlPointX = at(k);
        ctrlPointY = at(k + 1);
        currentX = at(k + 2);
        currentY = at(k + 3);
        break;
      case "q":
        path.rQuadTo(at(k), at(k + 1), at(k + 2), at(k + 3));
        ctrlPointX = f32(currentX + at(k));
        ctrlPointY = f32(currentY + at(k + 1));
        currentX = f32(currentX + at(k + 2));
        currentY = f32(currentY + at(k + 3));
        break;
      case "Q":
        path.quadTo(at(k), at(k + 1), at(k + 2), at(k + 3));
        ctrlPointX = at(k);
        ctrlPointY = at(k + 1);
        currentX = at(k + 2);
        currentY = at(k + 3);
        break;
      case "t":
        reflectiveCtrlPointX = 0;
        reflectiveCtrlPointY = 0;
        if ("qtQT".includes(previousCmd)) {
          reflectiveCtrlPointX = f32(currentX - ctrlPointX);
          reflectiveCtrlPointY = f32(currentY - ctrlPointY);
        }
        path.rQuadTo(reflectiveCtrlPointX, reflectiveCtrlPointY, at(k), at(k + 1));
        ctrlPointX = f32(currentX + reflectiveCtrlPointX);
        ctrlPointY = f32(currentY + reflectiveCtrlPointY);
        currentX = f32(currentX + at(k));
        currentY = f32(currentY + at(k + 1));
        break;
      case "T":
        reflectiveCtrlPointX = currentX;
        reflectiveCtrlPointY = currentY;
        if ("qtQT".includes(previousCmd)) {
          reflectiveCtrlPointX = f32(2 * currentX - ctrlPointX);
          reflectiveCtrlPointY = f32(2 * currentY - ctrlPointY);
        }
        path.quadTo(reflectiveCtrlPointX, reflectiveCtrlPointY, at(k), at(k + 1));
        ctrlPointX = reflectiveCtrlPointX;
        ctrlPointY = reflectiveCtrlPointY;
        currentX = at(k);
        currentY = at(k + 1);
        break;
      case "a":
        drawArc(path, currentX, currentY, f32(at(k + 5) + currentX), f32(at(k + 6) + currentY),
          at(k), at(k + 1), at(k + 2), at(k + 3) !== 0, at(k + 4) !== 0);
        currentX = f32(currentX + at(k + 5));
        currentY = f32(currentY + at(k + 6));
        ctrlPointX = currentX;
        ctrlPointY = currentY;
        break;
      case "A":
        drawArc(path, currentX, currentY, at(k + 5), at(k + 6),
          at(k), at(k + 1), at(k + 2), at(k + 3) !== 0, at(k + 4) !== 0);
        currentX = at(k + 5);
        currentY = at(k + 6);
        ctrlPointX = currentX;
        ctrlPointY = currentY;
        break;
    }
    previousCmd = cmd;
  }
  current[0] = currentX;
  current[1] = currentY;
  current[2] = ctrlPointX;
  current[3] = ctrlPointY;
}

/** Elliptical arc as cubic Béziers; shared with the Android 7.0 native parser. */
export function drawArc(
  p: PathSink,
  x0: number, y0: number, x1: number, y1: number,
  a: number, b: number, theta: number,
  isMoreThanHalf: boolean, isPositiveArc: boolean,
  segmentCount: (sweep: number) => number = javaSegmentCount,
): void {
  const thetaD = (theta / 180) * Math.PI;
  const cosTheta = Math.cos(thetaD);
  const sinTheta = Math.sin(thetaD);
  const x0p = (x0 * cosTheta + y0 * sinTheta) / a;
  const y0p = (-x0 * sinTheta + y0 * cosTheta) / b;
  const x1p = (x1 * cosTheta + y1 * sinTheta) / a;
  const y1p = (-x1 * sinTheta + y1 * cosTheta) / b;

  const dx = x0p - x1p;
  const dy = y0p - y1p;
  const xm = (x0p + x1p) / 2;
  const ym = (y0p + y1p) / 2;
  const dsq = dx * dx + dy * dy;
  if (dsq === 0.0) return; // Points are coincident.
  const disc = 1.0 / dsq - 1.0 / 4.0;
  if (disc < 0.0) {
    const adjust = f32(Math.sqrt(dsq) / 1.99999);
    drawArc(p, x0, y0, x1, y1, f32(a * adjust), f32(b * adjust), theta,
      isMoreThanHalf, isPositiveArc, segmentCount);
    return; // Points are too far apart.
  }
  const s = Math.sqrt(disc);
  const sdx = s * dx;
  const sdy = s * dy;
  let cx: number;
  let cy: number;
  if (isMoreThanHalf === isPositiveArc) {
    cx = xm - sdy;
    cy = ym + sdx;
  } else {
    cx = xm + sdy;
    cy = ym - sdx;
  }

  const eta0 = Math.atan2(y0p - cy, x0p - cx);
  const eta1 = Math.atan2(y1p - cy, x1p - cx);
  let sweep = eta1 - eta0;
  if (isPositiveArc !== sweep >= 0) {
    sweep += sweep > 0 ? -2 * Math.PI : 2 * Math.PI;
  }

  cx *= a;
  cy *= b;
  const tcx = cx;
  cx = cx * cosTheta - cy * sinTheta;
  cy = tcx * sinTheta + cy * cosTheta;

  arcToBezier(p, cx, cy, a, b, x0, y0, thetaD, eta0, sweep, segmentCount(sweep));
}

// Java: Math.abs((int) Math.ceil(sweep * 4 / Math.PI)), which rounds a
// negative sweep toward zero and so can use one segment fewer than C++.
function javaSegmentCount(sweep: number): number {
  return Math.abs(Math.trunc(Math.ceil((sweep * 4) / Math.PI)));
}

function arcToBezier(
  p: PathSink,
  cx: number, cy: number, a: number, b: number,
  e1x: number, e1y: number, theta: number, start: number, sweep: number,
  numSegments: number,
): void {
  let eta1 = start;
  const cosTheta = Math.cos(theta);
  const sinTheta = Math.sin(theta);
  const cosEta1 = Math.cos(eta1);
  const sinEta1 = Math.sin(eta1);
  let ep1x = -a * cosTheta * sinEta1 - b * sinTheta * cosEta1;
  let ep1y = -a * sinTheta * sinEta1 + b * cosTheta * cosEta1;

  const anglePerSegment = sweep / numSegments;
  for (let i = 0; i < numSegments; i++) {
    const eta2 = eta1 + anglePerSegment;
    const sinEta2 = Math.sin(eta2);
    const cosEta2 = Math.cos(eta2);
    const e2x = cx + a * cosTheta * cosEta2 - b * sinTheta * sinEta2;
    const e2y = cy + a * sinTheta * cosEta2 + b * cosTheta * sinEta2;
    const ep2x = -a * cosTheta * sinEta2 - b * sinTheta * cosEta2;
    const ep2y = -a * sinTheta * sinEta2 + b * cosTheta * cosEta2;
    const tanDiff2 = Math.tan((eta2 - eta1) / 2);
    const alpha = (Math.sin(eta2 - eta1) * (Math.sqrt(4 + 3 * tanDiff2 * tanDiff2) - 1)) / 3;
    const q1x = e1x + alpha * ep1x;
    const q1y = e1y + alpha * ep1y;
    const q2x = e2x - alpha * ep2x;
    const q2y = e2y - alpha * ep2y;
    p.cubicTo(f32(q1x), f32(q1y), f32(q2x), f32(q2y), f32(e2x), f32(e2y));
    eta1 = eta2;
    e1x = e2x;
    e1y = e2y;
    ep1x = ep2x;
    ep1y = ep2y;
  }
}
