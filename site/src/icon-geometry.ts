// How AdaptiveIconDrawable places layers and masks, from
// graphics/java/android/graphics/drawable/AdaptiveIconDrawable.java and
// core/res/res/values/config.xml on AOSP main. Layers are 108dp; the icon
// shows the central 72dp; launchers keep at least the 66dp safe zone.

/** Edge of every layer in dp (vdtoolkit::ADAPTIVE_ICON_SIZE). */
export const LAYER_DP = 108;
/** Edge of the visible icon in dp (vdtoolkit::ADAPTIVE_ICON_VISIBLE_DIAMETER). */
export const ICON_DP = 72;
/** Diameter of the safe zone in dp (vdtoolkit::ADAPTIVE_ICON_SAFE_ZONE). */
export const SAFE_ZONE_DP = 66;
/** Edge of a legacy icon in dp (vdtoolkit::LEGACY_ICON_SIZE). */
export const LEGACY_DP = 48;

/** AdaptiveIconDrawable.MASK_SIZE: mask paths are authored on a 100 unit square. */
export const MASK_SIZE = 100;

/** Layers are drawn 1 / DEFAULT_VIEW_PORT_SCALE times the icon bounds, centered. */
export const LAYER_SCALE = 1 + 2 * (1 / 4);

export interface Mask {
  id: string;
  label: string;
  /** SVG path data on the MASK_SIZE square. */
  path: string;
  source: string;
}

export const MASKS: Mask[] = [
  {
    id: "aosp",
    label: "Rounded square",
    path: "M50,0L92,0C96.42,0 100,4.58 100 8L100,92C100, 96.42 96.42 100 92 100L8 100C4.58, 100 0 96.42 0 92L0 8 C 0 4.42 4.42 0 8 0L50 0Z",
    source: "AOSP default, config_icon_mask",
  },
  {
    id: "circle",
    label: "Circle",
    path: "M50 0A50 50,0,1,1,50 100A50 50,0,1,1,50 0Z",
    source: "Pixel launchers",
  },
  {
    id: "squircle",
    label: "Squircle",
    path: "M50,0C10,0 0,10 0,50C0,90 10,100 50,100C90,100 100,90 100,50C100,10 90,0 50,0Z",
    source: "an approximation of Samsung One UI",
  },
  {
    id: "square",
    label: "Square",
    path: "M0,0L100,0L100,100L0,100Z",
    source: "launchers that apply no mask",
  },
];

export interface IconPlacement {
  /** Icon bounds in pixels. */
  size: number;
  /** Layer edge in pixels, LAYER_SCALE times the bounds. */
  layerSize: number;
  /** Offset of the layer's top-left from the icon's, so the layer is centered. */
  layerOffset: number;
  /** Safe zone diameter in pixels. */
  safeZone: number;
  /** Scale from mask units to pixels. */
  maskScale: number;
}

/** Placement for an icon whose bounds are `size` pixels square. */
export function placeIcon(size: number): IconPlacement {
  const layerSize = size * LAYER_SCALE;
  return {
    size,
    layerSize,
    layerOffset: (size - layerSize) / 2,
    safeZone: (size * SAFE_ZONE_DP) / ICON_DP,
    maskScale: size / MASK_SIZE,
  };
}
