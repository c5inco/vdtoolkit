// Port of VectorDrawable.VPathRenderer from Android 5.0 (android-5.0.0_r1),
// which drew vectors in Java before the native renderer existed. Two quirks
// matter for real drawables: a <clip-path> replaces the clip instead of
// intersecting with it, and nothing restores the clip when a <group> ends, so
// a clip stays active for everything drawn after it. Clips are not antialiased.

import type { CanvasKit, Canvas, Path } from "canvaskit-wasm";
import {
  buildPath, firstContour, groupMatrix, IDENTITY, multiply, scale, Scope, strokeCap, strokeJoin,
  transformPath, type Matrix,
} from "./canvas.ts";
import { createNodesFromPathData, nodesToPath, type PathDataNode } from "./path-data-api21.ts";
import { applyAlpha, DrawableLoadError } from "./values.ts";
import type { VectorDocument, VectorGroup, VectorNode } from "./vector-xml.ts";

interface Inflated {
  document: VectorDocument;
  nodes: Map<VectorNode, PathDataNode[]>;
}

/** VectorDrawable.inflate on API 21, which throws on anything it cannot read. */
export function inflateApi21(document: VectorDocument): Inflated {
  const nodes = new Map<VectorNode, PathDataNode[]>();
  const visit = (group: VectorGroup) => {
    for (const child of group.children) {
      if (child.kind === "group") {
        visit(child);
        continue;
      }
      if (child.kind === "path" && (child.fill?.kind === "gradient" || child.stroke?.kind === "gradient")) {
        throw new DrawableLoadError("Android 5.0 cannot load gradient colors; they need API 24");
      }
      if (child.pathData !== null) nodes.set(child, createNodesFromPathData(child.pathData));
    }
  };
  visit(document.root);
  return { document, nodes };
}

/** VectorDrawableState.updateCachedBitmap: draws into a bitmap the size of the bounds. */
export function drawApi21(ck: CanvasKit, scope: Scope, canvas: Canvas, inflated: Inflated, w: number, h: number): void {
  const { document, nodes } = inflated;
  const scaleX = Math.fround(w / document.viewportWidth);
  const scaleY = Math.fround(h / document.viewportHeight);
  const minScale = Math.min(scaleX, scaleY);
  let clip: Path | null = null;

  const withClip = (draw: () => void) => {
    canvas.save();
    if (clip) canvas.clipPath(clip, ck.ClipOp.Intersect, false);
    draw();
    canvas.restore();
  };

  const drawGroupTree = (group: VectorGroup, currentMatrix: Matrix) => {
    const stackedMatrix = multiply(currentMatrix, groupMatrix(group));
    for (const child of group.children) {
      if (child.kind === "group") {
        drawGroupTree(child, stackedMatrix);
        continue;
      }

      const finalPathMatrix = multiply(scale(scaleX, scaleY), stackedMatrix);
      let path = buildPath(ck, scope, (builder) => nodesToPath(nodes.get(child) ?? [], builder));

      if (child.kind === "clip-path") {
        clip = transformPath(ck, scope, path, finalPathMatrix);
        continue;
      }

      if (child.trimPathStart !== 0 || child.trimPathEnd !== 1) {
        let start = Math.fround((child.trimPathStart + child.trimPathOffset) % 1);
        let end = Math.fround((child.trimPathEnd + child.trimPathOffset) % 1);
        const measure = firstContour(ck, scope, path);
        const len = measure.length;
        start = Math.fround(start * len);
        end = Math.fround(end * len);
        path = buildPath(ck, scope, (builder) => {
          if (start > end) {
            measure.appendSegment(builder, start, len);
            measure.appendSegment(builder, 0, end);
          } else {
            measure.appendSegment(builder, start, end);
          }
          builder.rLineTo(0, 0); // fix bug in measure
        });
      }
      const renderPath = transformPath(ck, scope, path, finalPathMatrix);

      if (child.fill?.kind === "color" && child.fill.argb !== 0) {
        const fillPaint = scope.keep(new ck.Paint());
        fillPaint.setStyle(ck.PaintStyle.Fill);
        fillPaint.setAntiAlias(true);
        fillPaint.setColorInt(applyAlpha(child.fill.argb, child.fillAlpha));
        withClip(() => canvas.drawPath(renderPath, fillPaint));
      }

      if (child.stroke?.kind === "color" && child.stroke.argb !== 0) {
        const strokePaint = scope.keep(new ck.Paint());
        strokePaint.setStyle(ck.PaintStyle.Stroke);
        strokePaint.setAntiAlias(true);
        strokePaint.setStrokeJoin(strokeJoin(ck, child.strokeLineJoin));
        strokePaint.setStrokeCap(strokeCap(ck, child.strokeLineCap));
        strokePaint.setStrokeMiter(child.strokeMiterLimit);
        strokePaint.setColorInt(applyAlpha(child.stroke.argb, child.strokeAlpha));
        strokePaint.setStrokeWidth(Math.fround(child.strokeWidth * minScale));
        withClip(() => canvas.drawPath(renderPath, strokePaint));
      }
    }
  };

  drawGroupTree(document.root, IDENTITY);
}
