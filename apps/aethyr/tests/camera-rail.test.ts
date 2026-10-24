import assert from "node:assert/strict";
import test from "node:test";
import {
  CAMERA_RAIL_MAX_TIME_US,
  CAMERA_RAIL_SENSOR_HEIGHT_MM,
  cameraRailEase,
  frameIndexToPtsUs,
  prepareCameraRail,
  sampleCameraRail,
  type CameraRail,
  type CameraRailKey,
  type CameraRailVector,
} from "../packages/renderer/camera-rail.ts";

function near(a: number, b: number, epsilon = 1e-9) {
  assert.ok(Math.abs(a - b) <= epsilon, `${a} differs from ${b}`);
}
function vectorNear(a: CameraRailVector, b: CameraRailVector, epsilon = 1e-9) {
  for (let i = 0; i < 3; i++) near(a[i], b[i], epsilon);
}
function key(
  timeUs: number,
  position: CameraRailVector,
  extra: Partial<CameraRailKey> = {},
): CameraRailKey {
  return {
    timeUs,
    position,
    target: [0, 0, 0],
    verticalFovDegrees: 45,
    ...extra,
  };
}
function frameValid(sample: ReturnType<typeof sampleCameraRail>) {
  const delta = sample.target.map((n, i) => n - sample.position[i]);
  near(Math.hypot(...sample.up), 1);
  near(
    sample.up.reduce((sum, n, i) => sum + n * delta[i], 0),
    0,
    1e-8,
  );
  assert.ok(
    [
      ...sample.position,
      ...sample.target,
      ...sample.up,
      sample.verticalFovDegrees,
    ].every(Number.isFinite),
  );
}

test("a rail passes through every key, holds endpoints and supports random access", () => {
  const keys = [
    key(100_000, [0, 2, 10]),
    key(2_100_000, [5, 2, 8]),
    key(4_100_000, [8, 3, 3]),
    key(6_100_000, [9, 3, -2]),
  ];
  const rail = prepareCameraRail({ keys });
  for (const k of keys) {
    const sample = sampleCameraRail(rail, k.timeUs);
    vectorNear(sample.position, k.position);
    vectorNear(sample.target, k.target);
    assert.equal(sample.verticalFovDegrees, k.verticalFovDegrees);
    frameValid(sample);
  }
  assert.deepEqual(sampleCameraRail(rail, -1), sampleCameraRail(rail, 100_000));
  assert.deepEqual(
    sampleCameraRail(rail, CAMERA_RAIL_MAX_TIME_US),
    sampleCameraRail(rail, 6_100_000),
  );
  const middle = sampleCameraRail(rail, 3_100_000);
  for (const t of [6_100_000, 1_000_000, 0, 5_000_000, 2_100_000])
    frameValid(sampleCameraRail(rail, t));
  assert.deepEqual(sampleCameraRail(rail, 3_100_000), middle);
  assert.ok(
    Math.hypot(
      ...middle.position.map(
        (x, i) => x - (keys[1].position[i] + keys[2].position[i]) / 2,
      ),
    ) > 0.001,
  );
  for (const t of [2_100_000, 4_100_000]) {
    const before = sampleCameraRail(rail, t - 1).position;
    const after = sampleCameraRail(rail, t + 1).position;
    assert.ok(Math.hypot(...before.map((x, i) => x - after[i])) < 0.0001);
  }
});

test("two keys form a straight dolly and the outgoing key chooses easing", () => {
  for (const [ease, expectedX] of [
    ["linear", 2.5],
    ["in", 0.15625],
    ["out", 5.78125],
    ["in_out", 1.03515625],
  ] as const) {
    const rail = prepareCameraRail({
      keys: [
        key(0, [0, 2, 10], { ease }),
        key(1_000_000, [10, 2, 10], { ease: "out" }),
      ],
    });
    vectorNear(sampleCameraRail(rail, 250_000).position, [expectedX, 2, 10]);
  }
  for (const ease of ["linear", "in", "out", "in_out"] as const) {
    assert.equal(cameraRailEase(ease, -1), 0);
    assert.equal(cameraRailEase(ease, 2), 1);
  }
  near(cameraRailEase("in_out", 0.5), 0.5);
  assert.ok(cameraRailEase("in_out", 0.001) < 1e-7);
  assert.ok(1 - cameraRailEase("in_out", 0.999) < 1e-7);
});

test("centripetal knots handle uneven spacing and repeated spatial keys", () => {
  const positions = [
    [0, 0, 10],
    [10, 0, 10],
    [10.1, 0, 10],
    [20, 0, 10],
  ] as const;
  const rail = prepareCameraRail({
    keys: positions.map((p, i) => key(i * 1_000_000, p)),
  });
  for (let i = 0; i <= 100; i++) {
    const sample = sampleCameraRail(rail, 1_000_000 + i * 10_000);
    assert.ok(
      sample.position[0] >= 9.999999 && sample.position[0] <= 10.100001,
    );
    frameValid(sample);
  }
  const hold = prepareCameraRail({
    keys: [
      key(0, [0, 1, 5]),
      key(1_000_000, [2, 1, 5]),
      key(2_000_000, [2, 1, 5]),
      key(3_000_000, [8, 1, 5]),
    ],
  });
  for (const t of [1_000_001, 1_500_000, 1_999_999])
    vectorNear(sampleCameraRail(hold, t).position, [2, 1, 5]);
});

test("zoom interpolates focal length geometrically rather than FOV linearly", () => {
  const fov = (mm: number) =>
    (2 * Math.atan(CAMERA_RAIL_SENSOR_HEIGHT_MM / (2 * mm)) * 180) / Math.PI;
  const rail = prepareCameraRail({
    keys: [
      key(0, [0, 0, 5], { verticalFovDegrees: fov(24) }),
      key(1_000_000, [0, 0, 5], { verticalFovDegrees: fov(96) }),
    ],
  });
  near(sampleCameraRail(rail, 500_000).verticalFovDegrees, fov(48));
  assert.ok(
    Math.abs(
      sampleCameraRail(rail, 500_000).verticalFovDegrees -
        (fov(24) + fov(96)) / 2,
    ) > 1,
  );
  assert.equal(sampleCameraRail(rail, 0).verticalFovDegrees, fov(24));
  assert.equal(sampleCameraRail(rail, 1_000_000).verticalFovDegrees, fov(96));
});

test("explicit up and unwrapped roll survive sampling and endpoint holds", () => {
  const rail = prepareCameraRail({
    keys: [
      key(0, [0, 0, 5], { up: [0, 2, 1], rollDegrees: 0 }),
      key(1_000_000, [0, 0, 5], { up: [0, 2, 1], rollDegrees: 360 }),
    ],
  });
  vectorNear(sampleCameraRail(rail, 0).up, [0, 1, 0]);
  vectorNear(sampleCameraRail(rail, 250_000).up, [1, 0, 0]);
  vectorNear(sampleCameraRail(rail, 500_000).up, [0, -1, 0]);
  vectorNear(sampleCameraRail(rail, 1_000_001).up, [0, 1, 0]);
  const sideways = prepareCameraRail({
    keys: [key(0, [0, 0, 5], { up: [1, 0, 0] })],
  });
  vectorNear(sampleCameraRail(sideways, 100).up, [1, 0, 0]);
  const antipodal = prepareCameraRail({
    keys: [
      key(0, [0, 0, 5], { up: [0, 1, 0] }),
      key(1_000_000, [0, 0, 5], { up: [0, -1, 0] }),
    ],
  });
  vectorNear(sampleCameraRail(antipodal, 500_000).up, [1, 0, 0]);
});

test("up transport stays orthonormal through a vertical aim and a half turn", () => {
  const rail = prepareCameraRail({
    keys: [
      key(0, [0, 0, 5], { up: [1, 0, 0] }),
      key(1_000_000, [0, 5, 0], { up: [1, 0, 0] }),
      key(2_000_000, [0, 0, -5], { up: [1, 0, 0] }),
    ],
  });
  for (let t = 0; t <= 2_000_000; t += 5_000) {
    const sample = sampleCameraRail(rail, t);
    frameValid(sample);
    vectorNear(sample.up, [1, 0, 0]);
  }
});

test("preparation copies and freezes author input; cut boundaries remain separate", () => {
  const input: CameraRail = {
    keys: [key(0, [0, 1, 5]), key(1_000_000, [2, 1, 5])],
  };
  const rail = prepareCameraRail(input);
  const expected = sampleCameraRail(rail, 250_000);
  input.keys[0].position = [100, 100, 100];
  input.keys[0].timeUs = 200_000;
  assert.deepEqual(sampleCameraRail(rail, 250_000), expected);
  assert.ok(
    Object.isFrozen(rail) &&
      Object.isFrozen(rail.keys) &&
      Object.isFrozen(rail.keys[0].position),
  );
  const other = prepareCameraRail({ keys: [key(0, [100, 1, 5])] });
  vectorNear(sampleCameraRail(rail, 2_000_000).position, [2, 1, 5]);
  vectorNear(sampleCameraRail(other, 0).position, [100, 1, 5]);
});

test("invalid keys and degenerate sampled views fail before returning a pose", () => {
  const valid = key(0, [0, 0, 5]);
  for (const keys of [
    [],
    Array(2),
    Array.from({ length: 4097 }, (_, i) => key(i, [0, 0, 5])),
    [valid, valid],
    [valid, { ...valid, timeUs: -1 }],
  ])
    assert.throws(() => prepareCameraRail({ keys }));
  for (const patch of [
    { timeUs: 0.5 },
    { timeUs: Infinity },
    { timeUs: CAMERA_RAIL_MAX_TIME_US + 1 },
    { position: [0, NaN, 5] },
    { target: [20_001, 0, 0] },
    { position: [0, 0, 0] },
    { up: [0, 0, 1] },
    { up: [0, 0, 0] },
    { up: Array(3) },
    { verticalFovDegrees: 0 },
    { verticalFovDegrees: 180 },
    { verticalFovDegrees: NaN },
    { rollDegrees: Infinity },
    { rollDegrees: 36_001 },
    { ease: "bounce" },
  ])
    assert.throws(() =>
      prepareCameraRail({ keys: [{ ...valid, ...patch } as CameraRailKey] }),
    );
  const rail = prepareCameraRail({
    keys: [key(0, [-1, 0, 0]), key(1_000_000, [1, 0, 0])],
  });
  assert.throws(() => sampleCameraRail(rail, 500_000), /view/);
  for (const t of [NaN, Infinity, 0.5, CAMERA_RAIL_MAX_TIME_US + 1])
    assert.throws(() => sampleCameraRail(rail, t));
  assert.throws(() => cameraRailEase("linear", NaN));
});

test("integer PTS supports exact 60 fps and 30000/1001 without accumulated drift", () => {
  const sixty = { numerator: 60, denominator: 1 };
  assert.deepEqual(
    [0, 1, 2, 3, 60].map((k) => frameIndexToPtsUs(k, sixty)),
    [0, 16_667, 33_333, 50_000, 1_000_000],
  );
  const ntsc = { numerator: 30_000, denominator: 1_001 };
  assert.deepEqual(
    [0, 1, 2, 3, 30_000].map((k) => frameIndexToPtsUs(k, ntsc)),
    [0, 33_367, 66_733, 100_100, 1_001_000_000],
  );
  for (const rate of [sixty, ntsc])
    for (let k = 0; k < 100_000; k += 137) {
      const exact = ((k * rate.denominator) / rate.numerator) * 1e6;
      assert.ok(Math.abs(frameIndexToPtsUs(k, rate) - exact) <= 0.50001);
    }
  assert.equal(frameIndexToPtsUs(60 * 86_400, sixty), CAMERA_RAIL_MAX_TIME_US);
  assert.equal(frameIndexToPtsUs(1, { numerator: 128, denominator: 1 }), 7_813);
  assert.equal(
    frameIndexToPtsUs(60, { numerator: 120, denominator: 2 }),
    1_000_000,
  );
});

test("PTS rejects unsafe frames, unbounded rates and times beyond one day", () => {
  const rate = { numerator: 60, denominator: 1 };
  for (const k of [
    -1,
    1.5,
    NaN,
    Infinity,
    Number.MAX_SAFE_INTEGER + 1,
    Number.MAX_SAFE_INTEGER,
    60 * 86_400 + 1,
  ])
    assert.throws(() => frameIndexToPtsUs(k, rate));
  for (const bad of [
    { numerator: 0, denominator: 1 },
    { numerator: 1, denominator: 0 },
    { numerator: 0.5, denominator: 1 },
    { numerator: 241, denominator: 1 },
    { numerator: 1, denominator: 2 },
    { numerator: 1_000_001, denominator: 1_000_001 },
    { numerator: NaN, denominator: 1 },
  ])
    assert.throws(() => frameIndexToPtsUs(0, bad));
});
