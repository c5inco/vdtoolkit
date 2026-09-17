// CanvasKit helpers shared by the per-version renderers.

import type { CanvasKit, Path, PathBuilder, Shader } from "canvaskit-wasm";
import type { Gradient, VectorGroup } from "./vector-xml.ts";

const f32 = Math.fround;

/** A 3x3 matrix in row-major order, as CanvasKit takes it. */
export type Matrix = number[];

export const IDENTITY: Matrix = [1, 0, 0, 0, 1, 0, 0, 0, 1];

export function multiply(a: Matrix, b: Matrix): Matrix {
  const out = new Array<number>(9);
  for (let row = 0; row < 3; row++) {
    for (let column = 0; column < 3; column++) {
      out[row * 3 + column] = f32(
        a[row * 3] * b[column] + a[row * 3 + 1] * b[3 + column] + a[row * 3 + 2] * b[6 + column],
      );
    }
  }
  return out;
}

export function scale(sx: number, sy: number): Matrix {
  return [sx, 0, 0, 0, sy, 0, 0, 0, 1];
}

/**
 * Group::getLocalMatrix and VGroup.updateLocalMatrix, which agree on every
 * version: translate by -pivot, scale, rotate, translate by translate + pivot.
 */
export function groupMatrix(group: VectorGroup): Matrix {
  // SkMatrix::setRotate snaps values within 1/4096 of zero, so 90 degrees is exact.
  const radians = (group.rotation * Math.PI) / 180;
  const snap = (value: number) => (Math.abs(value) <= 1 / 4096 ? 0 : f32(value));
  const cos = snap(Math.cos(radians));
  const sin = snap(Math.sin(radians));
  const rotate = [cos, -sin, 0, sin, cos, 0, 0, 0, 1];
  let matrix = translate(-group.pivotX, -group.pivotY);
  matrix = multiply(scale(group.scaleX, group.scaleY), matrix);
  matrix = multiply(rotate, matrix);
  return multiply(
    translate(f32(group.translateX + group.pivotX), f32(group.translateY + group.pivotY)),
    matrix,
  );
}

function translate(dx: number, dy: number): Matrix {
  return [1, 0, dx, 0, 1, dy, 0, 0, 1];
}

/** Frees CanvasKit objects when a render finishes. */
export class Scope {
  private readonly objects: { delete(): void }[] = [];

  keep<T extends { delete(): void }>(object: T): T {
    this.objects.push(object);
    return object;
  }

  dispose(): void {
    for (const object of this.objects.reverse()) object.delete();
    this.objects.length = 0;
  }
}

export function buildPath(ck: CanvasKit, scope: Scope, draw: (builder: PathBuilder) => void): Path {
  const builder = new ck.PathBuilder();
  draw(builder);
  return scope.keep(builder.detachAndDelete());
}

export function transformPath(ck: CanvasKit, scope: Scope, path: Path, matrix: Matrix): Path {
  return buildPath(ck, scope, (builder) => builder.addPath(path, matrix));
}

/** SkPathMeasure: measures and cuts only the first contour of the path. */
export function firstContour(ck: CanvasKit, scope: Scope, path: Path) {
  const iterator = scope.keep(new ck.ContourMeasureIter(path, false, 1));
  const contour = iterator.next();
  if (contour) scope.keep(contour);
  return {
    length: contour ? contour.length() : 0,
    appendSegment(builder: PathBuilder, startD: number, stopD: number) {
      if (!contour) return;
      const segment = scope.keep(contour.getSegment(startD, stopD, true));
      builder.addPath(segment);
    },
  };
}

/** The shader GradientColor creates, in viewport coordinates. */
export function gradientShader(
  ck: CanvasKit,
  scope: Scope,
  gradient: Gradient,
  options: { premultiplied: boolean; localMatrix?: Matrix },
): Shader {
  const colors = Uint32Array.from(gradient.colors);
  const positions = gradient.offsets;
  const tileMode = [ck.TileMode.Clamp, ck.TileMode.Repeat, ck.TileMode.Mirror][gradient.tileMode] ?? ck.TileMode.Clamp;
  const flags = options.premultiplied ? 1 : 0; // SkGradientShader::kInterpolateColorsInPremul_Flag
  const matrix = options.localMatrix;
  switch (gradient.type) {
    case 0:
      return scope.keep(ck.Shader.MakeLinearGradient(
        [gradient.startX, gradient.startY], [gradient.endX, gradient.endY],
        colors, positions, tileMode, matrix, flags,
      ));
    case 1:
      return scope.keep(ck.Shader.MakeRadialGradient(
        [gradient.centerX, gradient.centerY], gradient.gradientRadius,
        colors, positions, tileMode, matrix, flags,
      ));
    default:
      return scope.keep(ck.Shader.MakeSweepGradient(
        gradient.centerX, gradient.centerY, colors, positions, ck.TileMode.Clamp, matrix ?? null, flags,
      ));
  }
}

export function strokeCap(ck: CanvasKit, cap: number) {
  return [ck.StrokeCap.Butt, ck.StrokeCap.Round, ck.StrokeCap.Square][cap] ?? ck.StrokeCap.Butt;
}

export function strokeJoin(ck: CanvasKit, join: number) {
  return [ck.StrokeJoin.Miter, ck.StrokeJoin.Round, ck.StrokeJoin.Bevel][join] ?? ck.StrokeJoin.Miter;
}
