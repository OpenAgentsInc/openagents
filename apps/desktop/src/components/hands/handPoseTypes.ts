export enum HandPose {
  NONE = "NONE",
  FIST = "FIST",
  OPEN_HAND = "OPEN_HAND",
  FLAT_HAND = "FLAT_HAND",
  TWO_FINGER_V = "TWO_FINGER_V",
  PINCH_CLOSED = "PINCH_CLOSED",
}

export interface Landmark {
  x: number;
  y: number;
  z: number;
  visibility?: number;
}

export type HandLandmarks = Landmark[];

export interface PinchCoordinates {
  x: number;
  y: number;
  z: number;
  normalizedMidX?: number;
  normalizedMidY?: number;
}