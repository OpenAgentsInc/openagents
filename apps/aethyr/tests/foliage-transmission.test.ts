import assert from "node:assert/strict";
import test from "node:test";
import type { VegetationTransmission } from "../packages/assets/src/scene.ts";
import {
  foliageAngularAttenuation,
  foliageTransmission,
  foliageTransmissionCode,
  WGSL_FOLIAGE_TRANSMISSION,
} from "../packages/renderer/foliage-transmission.ts";
import { MATERIAL } from "../packages/renderer/shaders.ts";
import { WGSL_CLUSTERED_LIGHTING } from "../packages/renderer/clustered-lighting.ts";

type V3 = readonly [number, number, number];
const normal: V3 = [0, 0, 1];
const backLight: V3 = [0, 0, -1];
const view: V3 = [0, 0, 1];
const color: V3 = [0.2, 0.5, 0.9];
function code(g: number) {
  return foliageTransmissionCode({
    model: "bounded-forward-sheet-v1",
    anisotropy: g,
  });
}
function near(a: number, b: number, epsilon = 1e-12) {
  assert.ok(Math.abs(a - b) <= epsilon, `${a} differs from ${b}`);
}

test("untagged transport retains unclamped .35 arithmetic and operand order", () => {
  assert.equal(foliageTransmissionCode(undefined), 0);
  for (let i = 0; i <= 100; i++) {
    const angle = (i * Math.PI) / 100;
    const light: V3 = [Math.sin(angle), 0, Math.cos(angle)];
    const subsurface: V3 = [-0.2, i * 0.01, 2.5];
    const mappedNormal: V3 = [0.6, 0, 0.8];
    const back = Math.max(
      -mappedNormal[0] * light[0] +
        -mappedNormal[1] * light[1] +
        -mappedNormal[2] * light[2],
      0,
    );
    assert.deepEqual(
      foliageTransmission(subsurface, mappedNormal, light, view),
      subsurface.map((value) => value * back * 0.35),
    );
  }
  assert.ok(
    WGSL_FOLIAGE_TRANSMISSION.includes(
      "if profileCode==0.0{return subsurface*max(dot(-n,l),0.0)*0.35;}",
    ),
  );
});

test("the isotropic opt-in is unit-Lambert transmission with bounded color", () => {
  const sample = foliageTransmission(color, normal, backLight, view, code(0));
  for (let c = 0; c < 3; c++) near(sample[c], color[c] / Math.PI);
  assert.deepEqual(
    foliageTransmission([-1, 0.5, 3], normal, backLight, view, code(0)),
    [0, 0.5 / Math.PI, 1 / Math.PI],
  );
  assert.deepEqual(
    foliageTransmission(color, normal, view, view, code(0)),
    [0, 0, 0],
  );
  assert.deepEqual(
    foliageTransmission([0, 0, 0], normal, backLight, view, code(0.8)),
    [0, 0, 0],
  );
});

test("forward angular transport attenuates other angles and never boosts Lambert", () => {
  for (const g of [0, 0.01, 0.35, 0.5, 0.8]) {
    assert.equal(foliageAngularAttenuation(-1, g), 1);
    for (let i = 0; i <= 400; i++) {
      const cosine = -1 + i / 200;
      const attenuation = foliageAngularAttenuation(cosine, g);
      assert.ok(
        Number.isFinite(attenuation) && attenuation >= 0 && attenuation <= 1,
      );
      const direction: V3 = [
        Math.sqrt(Math.max(0, 1 - cosine * cosine)),
        0,
        -cosine,
      ];
      const result = foliageTransmission(
        color,
        normal,
        backLight,
        direction,
        code(g),
      );
      for (let c = 0; c < 3; c++)
        assert.ok(result[c] >= 0 && result[c] <= color[c] / Math.PI + 1e-15);
    }
  }
  near(foliageAngularAttenuation(0.4, 0.5), 0.75 / 1.65 ** 1.5);
  assert.ok(
    foliageAngularAttenuation(0.4, 0.8) < foliageAngularAttenuation(0.4, 0.35),
  );
  assert.notEqual(
    foliageAngularAttenuation(0.9, 0.5),
    foliageAngularAttenuation(-0.9, 0.5),
  );
});

test("hemisphere-integrated output never exceeds the RGB transmission budget", () => {
  // Integrate output radiance*cos(theta_view)*dOmega for unit incident radiance.
  // The bounded angular factor makes this <= color*max(-N dot L,0).
  const albedo: V3 = [0.12, 0.6, 1];
  const polarSamples = 64,
    azimuthSamples = 96;
  for (const g of [0, 0.35, 0.5, 0.8])
    for (const cosineIncidence of [0, 0.05, 0.25, 0.5, 0.9, 1]) {
      const light: V3 = [
        Math.sqrt(1 - cosineIncidence ** 2),
        0,
        -cosineIncidence,
      ];
      const energy = [0, 0, 0];
      for (let i = 0; i < polarSamples; i++) {
        const mu = (i + 0.5) / polarSamples,
          radius = Math.sqrt(1 - mu * mu);
        for (let j = 0; j < azimuthSamples; j++) {
          const phi = ((j + 0.5) * 2 * Math.PI) / azimuthSamples;
          const outgoing: V3 = [
            radius * Math.cos(phi),
            radius * Math.sin(phi),
            mu,
          ];
          const radiance = foliageTransmission(
            albedo,
            normal,
            light,
            outgoing,
            code(g),
          );
          for (let c = 0; c < 3; c++)
            energy[c] +=
              (radiance[c] * mu * 2 * Math.PI) /
              (polarSamples * azimuthSamples);
        }
      }
      for (let c = 0; c < 3; c++) {
        assert.ok(
          energy[c] <= albedo[c] * cosineIncidence + 1e-12,
          `energy=${energy[c]}, g=${g}, mu=${cosineIncidence}`,
        );
        if (g === 0) near(energy[c], albedo[c] * cosineIncidence);
      }
    }
});

test("profile, packed-code and unit-direction guards reject unsupported inputs", () => {
  assert.equal(code(0), 1);
  near(code(0.8), 1.8);
  for (const invalid of [
    null,
    {},
    { model: "other", anisotropy: 0.5 },
    { model: "bounded-forward-sheet-v1", anisotropy: -1 },
    { model: "bounded-forward-sheet-v1", anisotropy: 0.80001 },
    { model: "bounded-forward-sheet-v1", anisotropy: NaN },
    { model: "bounded-forward-sheet-v1", anisotropy: Infinity },
    { model: "bounded-forward-sheet-v1", anisotropy: 0.5, thickness: 1 },
  ])
    assert.throws(() =>
      foliageTransmissionCode(invalid as VegetationTransmission),
    );
  for (const invalid of [-1, 0.5, 1.8001, NaN, Infinity])
    assert.throws(() =>
      foliageTransmission(color, normal, backLight, view, invalid),
    );
  for (const bad of [
    [0, 0, 0],
    [0, 0, 2],
    [NaN, 0, 1],
    [Infinity, 0, 1],
    Array(3),
  ]) {
    assert.throws(() =>
      foliageTransmission(
        color,
        bad as unknown as V3,
        backLight,
        view,
        code(0.5),
      ),
    );
    assert.throws(() =>
      foliageTransmission(color, normal, bad as unknown as V3, view, code(0.5)),
    );
    assert.throws(() =>
      foliageTransmission(
        color,
        normal,
        backLight,
        bad as unknown as V3,
        code(0.5),
      ),
    );
  }
  assert.throws(() =>
    foliageTransmission([NaN, 0, 1], normal, backLight, view, code(0.5)),
  );
  for (const bad of [NaN, Infinity, 1.00001, -1.00001])
    assert.throws(() => foliageAngularAttenuation(bad, 0.5));
});

test("sun, clustered lights and legacy locals share one helper without a layout expansion", () => {
  assert.equal((MATERIAL.match(/fn foliageTransmission\(/g) ?? []).length, 1);
  assert.ok(
    MATERIAL.includes(
      "foliageTransmission(subsurface,n,frame.sun.xyz,view,material.vegetationWaves.w)",
    ),
  );
  assert.ok(
    MATERIAL.includes(
      "foliageTransmission(subsurface,n,lightDirection,view,material.vegetationWaves.w)",
    ),
  );
  assert.ok(
    MATERIAL.includes(
      "clusteredSurfaceTransmission(v.clip.xy,v.world,n,view,base,rough,metal,surfaceSpecular,subsurface,geometricNormal,material.vegetationWaves.w)",
    ),
  );
  assert.ok(
    WGSL_CLUSTERED_LIGHTING.includes(
      "foliageTransmission(subsurface,n,l,view,transmissionProfile)",
    ),
  );
  assert.ok(WGSL_CLUSTERED_LIGHTING.includes("subsurface,geometric,0.0)"));
  assert.ok(Buffer.byteLength(MATERIAL, "utf8") < 65_536);
});
