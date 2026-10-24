import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import {
  compactSurfaceBrdf,
  WGSL_SURFACE_BRDF,
} from "../packages/renderer/surface-brdf.ts";
import { buildRendererShaders } from "../packages/renderer/shaders.ts";
import { selectedRendererFeatures as playerFeatures } from "../packages/renderer/player-features.ts";
import {
  buildClusteredLightingWGSL,
  WGSL_CLUSTERED_LIGHTING,
} from "../packages/renderer/clustered-lighting.ts";
import { compactGeneratedWGSL } from "../packages/renderer/wgsl-source.ts";
import {
  DEFAULT_EFFECT,
  materialEffect,
} from "../packages/editor-services/authoring.ts";

type V3 = [number, number, number];
type Terms = { nv: number; a2: number; viewRoot: number; f0: V3 };
const f = Math.fround;
const add = (a: number, b: number) => f(a + b);
const sub = (a: number, b: number) => f(a - b);
const mul = (a: number, b: number) => f(a * b);
const div = (a: number, b: number) => f(a / b);
const sqrt = (a: number) => f(Math.sqrt(a));
const pow = (a: number, b: number) => f(a ** b);
const map = (a: V3, op: (value: number, index: number) => number): V3 =>
  a.map(op) as V3;
const dot = (a: V3, b: V3) =>
  add(add(mul(a[0], b[0]), mul(a[1], b[1])), mul(a[2], b[2]));
function normalize(a: V3, fallback: V3): V3 {
  const lengthSquared = dot(a, a);
  return lengthSquared > f(0.000000000001)
    ? map(a, (v) => mul(v, div(1, sqrt(lengthSquared))))
    : fallback;
}
function fresnel(f0: V3, vh: number): V3 {
  const f90 = Math.min(1, Math.max(0, mul(Math.max(...f0), 50)));
  return map(f0, (value) =>
    add(value, mul(sub(f90 >= 1 ? 1 : f90, value), pow(sub(1, vh), 5))),
  );
}

// Frozen scalar translation of the original per-light expression. Every basic
// operation rounds to float32; this is arithmetic evidence, not GPU evidence.
function original(
  n: V3,
  v: V3,
  l: V3,
  base: V3,
  rough: number,
  metal: number,
  dielectric: number,
): V3 {
  const nl = Math.max(dot(n, l), 0),
    nv = Math.max(dot(n, v), f(0.001)),
    h = normalize(
      map(v, (value, i) => add(value, l[i])),
      n,
    ),
    nh = Math.max(dot(n, h), 0),
    vh = Math.max(dot(v, h), 0),
    a = mul(rough, rough),
    a2 = mul(a, a),
    d = div(
      a2,
      add(
        mul(f(3.14159265), pow(add(mul(mul(nh, nh), sub(a2, 1)), 1), 2)),
        f(0.00001),
      ),
    ),
    vis = div(
      0.5,
      Math.max(
        add(
          mul(nl, sqrt(add(mul(mul(nv, nv), sub(1, a2)), a2))),
          mul(nv, sqrt(add(mul(mul(nl, nl), sub(1, a2)), a2))),
        ),
        f(0.00001),
      ),
    ),
    f0 = map(base, (value) =>
      add(mul(dielectric, sub(1, metal)), mul(value, metal)),
    ),
    diffuseF = map(f0, (value) =>
      add(value, mul(sub(1, value), pow(sub(1, vh), 5))),
    ),
    specular = fresnel(f0, vh);
  return map(base, (value, i) =>
    mul(
      add(
        div(mul(mul(sub(1, diffuseF[i]), value), sub(1, metal)), f(3.14159265)),
        mul(mul(d, vis), specular[i]),
      ),
      nl,
    ),
  );
}

function prepare(n: V3, v: V3, b: V3, r: number, m: number, d: number): Terms {
  const nv = Math.max(dot(n, v), f(0.001)),
    a = mul(r, r),
    a2 = mul(a, a);
  return {
    nv,
    a2,
    viewRoot: sqrt(add(mul(mul(nv, nv), sub(1, a2)), a2)),
    f0: map(b, (value) => add(mul(d, sub(1, m)), mul(value, m))),
  };
}
function prepared(n: V3, v: V3, l: V3, b: V3, m: number, s: Terms): V3 {
  const nl = Math.max(dot(n, l), 0),
    h = normalize(
      map(v, (value, i) => add(value, l[i])),
      n,
    ),
    nh = Math.max(dot(n, h), 0),
    vh = Math.max(dot(v, h), 0),
    d = div(
      s.a2,
      add(
        mul(f(3.14159265), pow(add(mul(mul(nh, nh), sub(s.a2, 1)), 1), 2)),
        f(0.00001),
      ),
    ),
    vis = div(
      0.5,
      Math.max(
        add(
          mul(nl, s.viewRoot),
          mul(s.nv, sqrt(add(mul(mul(nl, nl), sub(1, s.a2)), s.a2))),
        ),
        f(0.00001),
      ),
    ),
    diffuseF = map(s.f0, (value) =>
      add(value, mul(sub(1, value), pow(sub(1, vh), 5))),
    ),
    specular = fresnel(s.f0, vh);
  return map(b, (value, i) =>
    mul(
      add(
        div(mul(mul(sub(1, diffuseF[i]), value), sub(1, m)), f(3.14159265)),
        mul(mul(d, vis), specular[i]),
      ),
      nl,
    ),
  );
}

test("prepared surface terms preserve float32 BRDF values across materials and light directions", () => {
  let seed = 0x2f19d51;
  const random = () => {
    seed = Math.imul(seed, 1664525) + 1013904223;
    return (seed >>> 0) / 4294967296;
  };
  const direction = () =>
    normalize(
      [f(random() * 2 - 1), f(random() * 2 - 1), f(random() * 2 - 1)],
      [0, 1, 0],
    );
  let comparisons = 0;
  for (const rough of [0.1, 0.100001, 0.35, 0.7, 1])
    for (const metal of [0, 0.001, 0.5, 0.999, 1])
      for (const dielectric of [0, 0.00001, 0.019999, 0.02, 0.04, 0.08, 1])
        for (let surface = 0; surface < 16; surface++) {
          const n = direction(),
            v = direction(),
            base: V3 = [
              f(random() * 4),
              f(random()),
              surface === 0 ? -0 : f(random()),
            ],
            terms = prepare(n, v, base, f(rough), f(metal), f(dielectric));
          for (const light of [
            n,
            map(n, (value) => -value),
            v,
            map(v, (value) => -value),
            direction(),
            direction(),
            direction(),
            direction(),
          ]) {
            assert.deepEqual(
              prepared(n, v, light, base, f(metal), terms),
              original(n, v, light, base, f(rough), f(metal), f(dielectric)),
            );
            comparisons++;
          }
        }
  assert.equal(comparisons, 22400);
});

test("WGSL hoists only light-independent terms and preserves light-dependent operand order", () => {
  const source = compactGeneratedWGSL(WGSL_SURFACE_BRDF);
  assert.ok(
    source.includes("let nv=max(dot(n,v),0.001);let a=r*r;let a2=a*a;"),
  );
  assert.ok(
    source.includes(
      "return BrdfTerms(nv,a2,sqrt(nv*nv*(1.0-a2)+a2),mix(vec3(d),b,m));",
    ),
  );
  assert.ok(
    source.includes(
      "let d=s.a2/(3.14159265*pow(nh*nh*(s.a2-1.0)+1.0,2.0)+0.00001);",
    ),
  );
  assert.ok(
    source.includes(
      "let vis=0.5/max(nl*s.viewRoot+s.nv*sqrt(nl*nl*(1.0-s.a2)+s.a2),0.00001);",
    ),
  );
  assert.ok(
    source.includes(
      "let diffuseF=s.f0+(vec3(1.0)-s.f0)*pow(1.0-vh,5.0);let f=directFresnel(s.f0,vh);",
    ),
  );
  assert.ok(
    source.includes(
      "return((vec3(1.0)-diffuseF)*b*(1.0-m)/3.14159265+d*vis*f)*nl;",
    ),
  );
  assert.ok(
    source.includes("return brdfLight(n,v,l,b,m,brdfTerms(n,v,b,r,m,d));"),
  );
});

test("clustered lights prepare once after zero-work rejection; sun and legacy retain the generic helper", () => {
  const { MATERIAL } = buildRendererShaders({ preparedSurfaceBrdf: true }),
    fragment = MATERIAL.slice(
      MATERIAL.indexOf("fn shade(v:Out"),
      MATERIAL.indexOf("@fragment fn fs(v:Out"),
    );
  assert.equal((fragment.match(/\bbt\(/g) ?? []).length, 0);
  assert.ok(
    fragment.includes(
      "brdf(n,view,frame.sun.xyz,base,rough,metal,surfaceSpecular)",
    ),
  );
  assert.ok(
    fragment.includes(
      "brdf(n,view,lightDirection,base,rough,metal,surfaceSpecular)",
    ),
  );
  const cluster = compactGeneratedWGSL(buildClusteredLightingWGSL(true)),
    loop = cluster.slice(cluster.indexOf("for(var entry=")),
    prepareAt = cluster.indexOf(
      "let bs=brdfTerms(n,view,base,rough,metal,dielectric);",
    );
  assert.ok(prepareAt > cluster.indexOf("if total==0u{return vec3(0.0);}"));
  assert.ok(prepareAt > cluster.indexOf("if iterations==0u{return color;}"));
  assert.ok(prepareAt < cluster.indexOf("for(var entry="));
  assert.equal((cluster.match(/\bbrdfTerms\(/g) ?? []).length, 1);
  assert.ok(loop.includes("brdfLight(n,view,l,base,metal,bs)"));
  assert.ok(!loop.includes("brdfTerms("));
  assert.match(
    MATERIAL,
    /fn brdf\(n:vec3<f32>,v:vec3<f32>,l:vec3<f32>,b:vec3<f32>,r:f32,m:f32,d:f32\)/,
  );
  assert.ok(Buffer.byteLength(MATERIAL) <= 65536);
});

test("owned helper compaction respects identifier boundaries and retains the generic wrapper", () => {
  assert.equal(
    compactSurfaceBrdf(
      "BrdfTerms brdfTerms brdfLight brdfTermsExtra _brdfLight brdf viewRoot diffuseF transmissionProfile normalUV normalScale textureAlpha sourceNormal sourceNormalFrame skyFill normalUVExtra",
    ),
    "BT bt bl brdfTermsExtra _brdfLight brdf vr df tp nu ns textureAlpha sn sourceNormalFrame sf normalUVExtra",
  );
  assert.equal(
    compactSurfaceBrdf(compactSurfaceBrdf(WGSL_SURFACE_BRDF)),
    compactSurfaceBrdf(WGSL_SURFACE_BRDF),
  );
});

test("generated material retains the original source headroom for creator edits", () => {
  for (const preparedSurfaceBrdf of [false, true]) {
    const { MATERIAL } = buildRendererShaders({
      ...playerFeatures,
      preparedSurfaceBrdf,
    });
    assert.ok(Buffer.byteLength(MATERIAL) <= 65302);
    assert.equal(MATERIAL.split("var textureAlpha=").length, 2);
    const edited = materialEffect(DEFAULT_EFFECT, MATERIAL);
    assert.ok(
      edited.includes(
        "base=clamp(oa_material(base,v.world,v.normal.xyz,v.rawUV,frame.eye.w)",
      ),
    );
    assert.ok(Buffer.byteLength(edited) <= 65536);
  }
});

test("default original and explicit prepared shaders retain their frozen comparison identities", () => {
  const original = buildRendererShaders(playerFeatures).MATERIAL,
    disabled = buildRendererShaders({
      ...playerFeatures,
      preparedSurfaceBrdf: false,
    }).MATERIAL,
    prepared = buildRendererShaders({
      ...playerFeatures,
      preparedSurfaceBrdf: true,
    }).MATERIAL,
    hash = (value: string) => createHash("sha256").update(value).digest("hex");
  assert.equal(disabled, original);
  assert.equal(Buffer.byteLength(original), 65302);
  assert.equal(
    hash(original),
    "7300aa644c4db11a64de6e457c8323c82767a44f3e439306e3bd4e944a47c40c",
  );
  assert.equal(Buffer.byteLength(prepared), 65285);
  assert.equal(
    hash(prepared),
    "99bb089526fdf0ef6b55ae88955415489d24b161ab8330e9b5d294fec67f2b78",
  );
  assert.equal(WGSL_CLUSTERED_LIGHTING, buildClusteredLightingWGSL(false));
  assert.ok(!original.includes("struct BT{"));
  assert.ok(!original.includes("let bs=bt("));
  assert.ok(original.includes("brdf(n,view,l,base,rough,metal,dielectric)"));
  assert.ok(prepared.includes("struct BT{"));
  assert.ok(
    prepared.includes("let bs=bt(n,view,base,rough,metal,dielectric);"),
  );
});
