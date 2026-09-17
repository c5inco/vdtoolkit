// Port of libs/hwui/VectorDrawable.cpp from Android 7.0 (android-7.0.0_r1),
// the first native renderer. Paths are transformed to pixels before drawing,
// stroke width is scaled by the smaller axis of the group transform, clips are
// not antialiased, and gradients interpolate unpremultiplied colors.

import type { Canvas, CanvasKit } from "canvaskit-wasm";
import {
  buildPath, firstContour, gradientShader, groupMatrix, IDENTITY, multiply, scale, Scope, strokeCap,
  strokeJoin, transformPath, type Matrix,
} from "./canvas.ts";
import { getPathDataFromAsciiString, verbsToPath, type PathData } from "./path-data-hwui.ts";
import { applyAlpha } from "./values.ts";
import type { VectorDocument, VectorGroup, VectorNode, VectorPath } from "./vector-xml.ts";

export interface InflatedHwui {
  document: VectorDocument;
  data: Map<VectorNode, PathData>;
}

/** Java inflation from API 24: path data that fails to parse throws. */
export function inflateHwui(document: VectorDocument): InflatedHwui {
  const data = new Map<VectorNode, PathData>();
  const visit = (group: VectorGroup) => {
    for (const child of group.children) {
      if (child.kind === "group") visit(child);
      else if (child.pathData !== null) data.set(child, getPathDataFromAsciiString(child.pathData));
    }
  };
  visit(document.root);
  return { document, data };
}

/** Tree::updateBitmapCache. */
export function drawApi24(ck: CanvasKit, scope: Scope, canvas: Canvas, inflated: InflatedHwui, w: number, h: number): void {
  const { document, data } = inflated;
  const scaleX = Math.fround(w / document.viewportWidth);
  const scaleY = Math.fround(h / document.viewportHeight);

  const drawGroup = (group: VectorGroup, currentMatrix: Matrix) => {
    const stackedMatrix = multiply(currentMatrix, groupMatrix(group));
    canvas.save();
    for (const child of group.children) {
      if (child.kind === "group") {
        drawGroup(child, stackedMatrix);
        continue;
      }

      // Path::draw
      const matrixScale = getMatrixScale(stackedMatrix);
      if (matrixScale === 0) continue;
      const pathMatrix = multiply(scale(scaleX, scaleY), stackedMatrix);
      let path = buildPath(ck, scope, (builder) => {
        const pathData = data.get(child);
        if (pathData) verbsToPath(builder, pathData, "bezier");
      });
      if (child.kind === "path") path = applyTrim(ck, scope, path, child);
      const renderPath = transformPath(ck, scope, path, pathMatrix);
      const strokeScale = Math.fround(Math.min(scaleX, scaleY) * matrixScale);

      if (child.kind === "clip-path") {
        canvas.clipPath(renderPath, ck.ClipOp.Intersect, false);
      } else {
        drawFullPath(child, renderPath, strokeScale, pathMatrix);
      }
    }
    canvas.restore();
  };

  // FullPath::drawPath. One SkPaint serves fill and stroke, so a fill gradient
  // stays on the paint when the stroke is a plain color.
  const drawFullPath = (node: VectorPath, renderPath: ReturnType<typeof transformPath>, strokeScale: number, matrix: Matrix) => {
    const paint = scope.keep(new ck.Paint());
    let needsFill = false;
    if (node.fill?.kind === "gradient") {
      paint.setColorInt(applyAlpha(0xff000000, node.fillAlpha));
      paint.setShader(gradientShader(ck, scope, node.fill.gradient, { premultiplied: false, localMatrix: matrix }));
      needsFill = true;
    } else if (node.fill?.kind === "color" && node.fill.argb !== 0) {
      paint.setColorInt(applyAlpha(node.fill.argb, node.fillAlpha));
      needsFill = true;
    }
    if (needsFill) {
      paint.setStyle(ck.PaintStyle.Fill);
      paint.setAntiAlias(true);
      renderPath = withFillType(ck, scope, renderPath, node.fillType);
      canvas.drawPath(renderPath, paint);
    }

    let needsStroke = false;
    if (node.stroke?.kind === "gradient") {
      paint.setColorInt(applyAlpha(0xff000000, node.strokeAlpha));
      paint.setShader(gradientShader(ck, scope, node.stroke.gradient, { premultiplied: false, localMatrix: matrix }));
      needsStroke = true;
    } else if (node.stroke?.kind === "color" && node.stroke.argb !== 0) {
      paint.setColorInt(applyAlpha(node.stroke.argb, node.strokeAlpha));
      needsStroke = true;
    }
    if (needsStroke) {
      paint.setStyle(ck.PaintStyle.Stroke);
      paint.setAntiAlias(true);
      paint.setStrokeJoin(strokeJoin(ck, node.strokeLineJoin));
      paint.setStrokeCap(strokeCap(ck, node.strokeLineCap));
      paint.setStrokeMiter(node.strokeMiterLimit);
      paint.setStrokeWidth(Math.fround(node.strokeWidth * strokeScale));
      canvas.drawPath(renderPath, paint);
    }
  };

  drawGroup(document.root, IDENTITY);
}

// Path::getMatrixScale: the scale a stroke gets from the group transforms.
function getMatrixScale(matrix: Matrix): number {
  const v0 = [matrix[1], matrix[4]]; // maps (0, 1)
  const v1 = [matrix[0], matrix[3]]; // maps (1, 0)
  const scaleX = Math.fround(Math.hypot(v0[0], v0[1]));
  const scaleY = Math.fround(Math.hypot(v1[0], v1[1]));
  const crossProduct = Math.fround(v0[0] * v1[1] - v0[1] * v1[0]);
  const maxScale = Math.max(scaleX, scaleY);
  return maxScale > 0 ? Math.fround(Math.abs(crossProduct) / maxScale) : 0;
}

/** applyTrim, unchanged from 7.0 to main. */
export function applyTrim(ck: CanvasKit, scope: Scope, inPath: ReturnType<typeof buildPath>, node: VectorPath) {
  const { trimPathStart, trimPathEnd, trimPathOffset } = node;
  if (trimPathStart === 0 && trimPathEnd === 1) return inPath;
  if (trimPathStart === trimPathEnd) return buildPath(ck, scope, () => {});
  const measure = firstContour(ck, scope, inPath);
  const len = Math.fround(measure.length);
  const start = Math.fround(len * Math.fround((trimPathStart + trimPathOffset) % 1));
  const end = Math.fround(len * Math.fround((trimPathEnd + trimPathOffset) % 1));
  return buildPath(ck, scope, (builder) => {
    if (start > end) {
      measure.appendSegment(builder, start, len);
      if (end > 0) measure.appendSegment(builder, 0, end);
    } else {
      measure.appendSegment(builder, start, end);
    }
  });
}

export function withFillType(ck: CanvasKit, scope: Scope, path: ReturnType<typeof buildPath>, fillType: number) {
  const copy = scope.keep(path.copy());
  copy.setFillType(fillType === 1 ? ck.FillType.EvenOdd : ck.FillType.Winding);
  return copy;
}
