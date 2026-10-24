import assert from "node:assert/strict";
import test from "node:test";
import {
  cameraForCapture,
  environmentForCapture,
  FilmSessionState,
  validateFilmCaptureSize,
  validateFrameCaptureOptions,
  validateFramePixelOptions,
} from "../packages/renderer/film-frame.ts";
import { direction } from "../packages/renderer/math.ts";
import {
  DEFAULT_ENVIRONMENT,
  validateEnvironment,
} from "../packages/editor-services/protocol.ts";

test("transient film poses preserve roll, lens and selected vignette without changing live state", () => {
  const base = {
    eye: [1, 2, 3] as [number, number, number],
    yaw: 0.2,
    pitch: 0.3,
    fov: 0.6,
    up: [0, 1, 0] as [number, number, number],
    vignette: {
      shape: "cosine-fourth" as const,
      intensity: 0.4,
      blendWeight: 1,
    },
  };
  const before = structuredClone(base),
    pose = {
      eye: [5, 6, 7] as [number, number, number],
      target: [5, 6, 6] as [number, number, number],
      up: [1, 0, 0] as [number, number, number],
      fov: 0.5,
    };
  const capture = cameraForCapture(base, pose);
  assert.deepEqual(capture.up, [1, 0, 0]);
  assert.deepEqual(direction(capture.yaw, capture.pitch), [0, 0, -1]);
  assert.equal(capture.fov, 0.5);
  assert.deepEqual(capture.vignette, base.vignette);
  assert.deepEqual(base, before);
  capture.eye[0] = 999;
  capture.up![0] = 0;
  capture.vignette!.intensity = 0;
  assert.deepEqual(base, before);
  assert.equal(pose.eye[0], 5);
  assert.equal(pose.up[0], 1);
  assert.equal(cameraForCapture(base, { ...pose, fov: undefined }).fov, 0.6);
  assert.equal(
    cameraForCapture(base, { ...pose, vignette: null }).vignette,
    undefined,
  );
});

test("frame options reject degenerate orientation and invalid times, lenses and formats", () => {
  const camera = {
    eye: [0, 0, 0],
    target: [0, 0, -1],
    up: [0, 1, 0],
    fov: 0.5,
  };
  for (const time of [0, 181.783333, 86400])
    assert.equal(
      validateFrameCaptureOptions({ time, camera, format: "rgba" }).time,
      time,
    );
  for (const bad of [
    { time: -1 },
    { time: 86400.1 },
    { time: NaN },
    { time: Infinity },
    { exposure: 0 },
    { exposure: 0.009 },
    { exposure: 64.001 },
    { exposure: NaN },
    { exposure: Infinity },
    { exposure: "1" },
    { exposure: null },
    { format: "jpeg" },
    { width: 1280 },
    { camera: { ...camera, eye: [NaN, 0, 0] } },
    { camera: { ...camera, target: [0, 0, 0] } },
    { camera: { ...camera, up: [0, 0, 0] } },
    { camera: { ...camera, up: [0, 0, 1] } },
    { camera: { ...camera, fov: 0.1 } },
    { camera: { ...camera, fov: 3 } },
    {
      camera: {
        ...camera,
        vignette: { shape: "unknown", intensity: 1, blendWeight: 1 },
      },
    },
  ])
    assert.throws(() => validateFrameCaptureOptions(bad));
  assert.deepEqual(validateFrameCaptureOptions({}), {});
  assert.deepEqual(validateFramePixelOptions({ time: 0, exposure: 16 }), {
    time: 0,
    exposure: 16,
  });
  assert.throws(() => validateFramePixelOptions({ format: "rgba" }));
  assert.deepEqual(validateFilmCaptureSize({ width: 1280, height: 720 }), {
    width: 1280,
    height: 720,
  });
  assert.deepEqual(validateFilmCaptureSize({ width: 1920, height: 1080 }), {
    width: 1920,
    height: 1080,
  });
  for (const bad of [
    { width: 1921, height: 1080 },
    { width: 1280, height: 1081 },
    { width: 0, height: 720 },
    { width: 1280.5, height: 720 },
    { width: NaN, height: 720 },
    { width: 1280, height: 720, frames: 10907 },
  ])
    assert.throws(() => validateFilmCaptureSize(bad));
});

test("capture exposure overrides the whole environment once without changing live lighting", () => {
  const base = { ...DEFAULT_ENVIRONMENT, exposure: 3 },
    before = structuredClone(base);
  for (const exposure of [0.01, 0.1, 1, 16, 64]) {
    assert.equal(validateFrameCaptureOptions({ exposure }).exposure, exposure);
    const env = environmentForCapture(base, exposure);
    assert.deepEqual(env, { ...before, exposure });
    assert.notEqual(env, base);
    env.exposure = 0.5;
    env.sunIntensity = 999;
    assert.deepEqual(base, before);
  }
  assert.deepEqual(environmentForCapture(base), before);
  assert.notEqual(environmentForCapture(base), base);
  assert.throws(() => environmentForCapture(base, 65), /frame exposure/);
  assert.throws(() => validateEnvironment({ ...base, exposure: 16 }));
  assert.throws(() => validateEnvironment({ ...base, exposure: 0.01 }));
  assert.deepEqual(base, before);
});

test("manual policy blocks live ticks and input throughout a bounded one-flight session", () => {
  const state = new FilmSessionState();
  let kernelSteps = 0;
  const tick = () => {
    if (state.liveAllowed) kernelSteps++;
  };
  tick();
  state.begin({ width: 1280, height: 720 });
  assert.equal(state.manual, true);
  tick();
  assert.equal(kernelSteps, 1);
  assert.throws(() => state.begin({ width: 1280, height: 720 }));
  state.enterFrame();
  assert.throws(() => state.enterFrame());
  assert.throws(() => state.end());
  tick();
  state.finishFrame();
  tick();
  assert.equal(kernelSteps, 1);
  state.end();
  tick();
  assert.equal(kernelSteps, 2);
  state.end();
  assert.equal(state.manual, false);
  state.enterFrame();
  assert.equal(state.liveAllowed, false);
  assert.throws(() => state.begin({ width: 1, height: 1 }));
  state.finishFrame();
  assert.equal(state.liveAllowed, true);
});
