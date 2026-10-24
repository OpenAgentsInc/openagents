/** Shared CPU/WGSL frame layout. All offsets count float32 values. */
export const LOCAL_POINT_SHADOWS = 4;
export const LOCAL_SPOT_SHADOWS = 16;
export const SPOT_SHADOW_START = LOCAL_POINT_SHADOWS * 6;
export const LOCAL_SHADOW_VIEWS = SPOT_SHADOW_START + LOCAL_SPOT_SHADOWS;
const skyRotation = 608 + LOCAL_SHADOW_VIEWS * 16;
export const FRAME_OFFSETS = {
  skyRotation,
  skyUV: skyRotation + 16,
  skyTint: skyRotation + 20,
  skyEnabled: skyRotation + 24,
  iblSH: skyRotation + 28,
  iblRotation: skyRotation + 64,
  iblSettings: skyRotation + 80,
  probes: skyRotation + 84,
  clipPlane: skyRotation + 100,
  passSettings: skyRotation + 104,
  iblGain: skyRotation + 108,
  bounceSettings: skyRotation + 112,
  volumeMedium: skyRotation + 116,
  volumeRange: skyRotation + 120,
  distantShadow: skyRotation + 124,
  iblConstantSH: skyRotation + 140,
  skyLayers: skyRotation + 176,
} as const;
export const FRAME_FLOAT_COUNT = skyRotation + 180;
