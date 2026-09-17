// Port of libs/hwui/VectorDrawable.cpp on AOSP main, unchanged in rendering
// since Android 10. Groups concatenate onto the canvas, so strokes scale with
// the canvas; clips are antialiased; gradients interpolate premultiplied
// colors; arcs come from SkPath::arcTo.

import type { Canvas, CanvasKit } from "canvaskit-wasm";
import { buildPath, gradientShader, groupMatrix, Scope, strokeCap, strokeJoin } from "./canvas.ts";
import { verbsToPath } from "./path-data-hwui.ts";
import { applyTrim, withFillType, type InflatedHwui } from "./render-api24.ts";
import { applyAlpha } from "./values.ts";
import type { VectorGroup, VectorPath } from "./vector-xml.ts";

/** Tree::updateBitmapCache. */
export function drawLatest(ck: CanvasKit, scope: Scope, canvas: Canvas, inflated: InflatedHwui, w: number, h: number): void {
  const { document, data } = inflated;
  canvas.save();
  canvas.scale(Math.fround(w / document.viewportWidth), Math.fround(h / document.viewportHeight));

  const drawGroup = (group: VectorGroup) => {
    canvas.save();
    canvas.concat(groupMatrix(group));
    for (const child of group.children) {
      if (child.kind === "group") {
        drawGroup(child);
        continue;
      }
      const path = buildPath(ck, scope, (builder) => {
        const pathData = data.get(child);
        if (pathData) verbsToPath(builder, pathData, "skia");
      });
      if (child.kind === "clip-path") {
        canvas.clipPath(path, ck.ClipOp.Intersect, true);
      } else {
        drawFullPath(child, applyTrim(ck, scope, path, child));
      }
    }
    canvas.restore();
  };

  // FullPath::draw. One SkPaint serves fill and stroke, so a fill gradient
  // stays on the paint when the stroke is a plain color.
  const drawFullPath = (node: VectorPath, renderPath: ReturnType<typeof buildPath>) => {
    if (node.fill !== null && (node.fill.kind === "gradient" || node.fill.argb !== 0)) {
      renderPath = withFillType(ck, scope, renderPath, node.fillType);
    }
    const paint = scope.keep(new ck.Paint());
    let needsFill = false;
    if (node.fill?.kind === "gradient") {
      paint.setColorInt(applyAlpha(0xff000000, node.fillAlpha));
      paint.setShader(gradientShader(ck, scope, node.fill.gradient, { premultiplied: true }));
      needsFill = true;
    } else if (node.fill?.kind === "color" && node.fill.argb !== 0) {
      paint.setColorInt(applyAlpha(node.fill.argb, node.fillAlpha));
      needsFill = true;
    }
    if (needsFill) {
      paint.setStyle(ck.PaintStyle.Fill);
      paint.setAntiAlias(true);
      canvas.drawPath(renderPath, paint);
    }

    let needsStroke = false;
    if (node.stroke?.kind === "gradient") {
      paint.setColorInt(applyAlpha(0xff000000, node.strokeAlpha));
      paint.setShader(gradientShader(ck, scope, node.stroke.gradient, { premultiplied: true }));
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
      paint.setStrokeWidth(node.strokeWidth);
      canvas.drawPath(renderPath, paint);
    }
  };

  drawGroup(document.root);
  canvas.restore();
}
