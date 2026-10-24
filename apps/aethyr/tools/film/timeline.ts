import {
  validateFilmCaptureSize,
  validateFrameCaptureOptions,
  type FilmCameraPose,
  type FrameCaptureOptions,
} from "../../packages/renderer/film-frame.ts";
import { frameIndexToPtsUs } from "../../packages/renderer/camera-rail.ts";

export interface FilmFrame {
  ptsUs: number;
  worldTimeSeconds: number;
  /** Absolute capture-only exposure multiplier, independent of editorial fade. */
  exposure?: number;
  transforms?: FrameCaptureOptions["transforms"];
  camera?: FilmCameraPose;
  shotId?: string;
  black?: boolean;
  holdPrevious?: boolean;
  /** This editorial fade scales the final sRGB display values. */
  fade?: number;
  letterbox?: { top: number; bottom: number };
}
export interface FilmTimeline {
  schema: "aethyrforge.film.v1";
  width: number;
  height: number;
  frameRate: { numerator: number; denominator: number };
  frames: FilmFrame[];
}
export function validateFilmTimeline(value: unknown): FilmTimeline {
  const film = value as FilmTimeline;
  if (!film || film.schema !== "aethyrforge.film.v1")
    throw Error("Choose an Aethyrforge film timeline.");
  validateFilmCaptureSize({ width: film.width, height: film.height });
  if (film.width % 2 || film.height % 2)
    throw Error("Choose even film dimensions for H.264 output.");
  if (
    !Array.isArray(film.frames) ||
    !film.frames.length ||
    film.frames.length > 216000
  )
    throw Error("Use one to 216,000 film frames.");
  for (let i = 0; i < film.frames.length; i++) {
    const frame = film.frames[i];
    if (!frame || frame.ptsUs !== frameIndexToPtsUs(i, film.frameRate))
      throw Error(`Frame ${i} needs its exact output timestamp.`);
    validateFrameCaptureOptions({
      time: frame.worldTimeSeconds,
      camera: frame.camera,
      exposure: frame.exposure,
      transforms: frame.transforms,
    });
    if (
      [frame.black, frame.holdPrevious].some(
        (value) => value !== undefined && typeof value !== "boolean",
      )
    )
      throw Error(`Frame ${i} needs boolean black and hold flags.`);
    if (!Number.isFinite(frame.worldTimeSeconds))
      throw Error(`Frame ${i} needs an explicit world time.`);
    if (!frame.black && !frame.camera)
      throw Error(`Frame ${i} needs a camera or a black frame.`);
    if (frame.shotId !== undefined && !/^MT-\d{2}$/.test(frame.shotId))
      throw Error(`Frame ${i} names an unsupported reference camera.`);
    if (
      frame.fade !== undefined &&
      (!Number.isFinite(frame.fade) || frame.fade < 0 || frame.fade > 1)
    )
      throw Error(`Frame ${i} needs a fade between zero and one.`);
    if (
      frame.letterbox &&
      (!Number.isInteger(frame.letterbox.top) ||
        !Number.isInteger(frame.letterbox.bottom) ||
        frame.letterbox.top < 0 ||
        frame.letterbox.bottom < 0 ||
        frame.letterbox.top + frame.letterbox.bottom >= film.height)
    )
      throw Error(`Frame ${i} needs bounded letterbox rows.`);
    if (
      frame.holdPrevious &&
      (!i ||
        frame.black ||
        film.frames[i - 1].black ||
        frame.worldTimeSeconds !== film.frames[i - 1].worldTimeSeconds ||
        frame.exposure !== film.frames[i - 1].exposure ||
        JSON.stringify(frame.transforms) !==
          JSON.stringify(film.frames[i - 1].transforms) ||
        JSON.stringify(frame.camera) !==
          JSON.stringify(film.frames[i - 1].camera) ||
        (frame.shotId !== undefined &&
          frame.shotId !== film.frames[i - 1].shotId))
    )
      throw Error(`Frame ${i} cannot hold a different scene sample.`);
  }
  return film;
}
/** Editorial operations never change the captured world sample or its stored bytes. */
export function editorialFrame(
  raw: Uint8Array,
  frame: FilmFrame,
  width: number,
  height: number,
) {
  if (raw.length !== width * height * 4)
    throw Error("The film frame has an unexpected byte count.");
  const result = new Uint8Array(raw);
  const fade = frame.black ? 0 : (frame.fade ?? 1);
  if (fade !== 1)
    for (let i = 0; i < result.length; i += 4) {
      result[i] = Math.round(result[i] * fade);
      result[i + 1] = Math.round(result[i + 1] * fade);
      result[i + 2] = Math.round(result[i + 2] * fade);
    }
  const top = frame.letterbox?.top ?? 0,
    bottom = frame.letterbox?.bottom ?? 0;
  for (const [begin, end] of [
    [0, top * width * 4],
    [(height - bottom) * width * 4, result.length],
  ]) {
    for (let i = begin; i < end; i += 4)
      result[i] = result[i + 1] = result[i + 2] = 0;
  }
  return result;
}
