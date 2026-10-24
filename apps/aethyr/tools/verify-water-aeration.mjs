// Original fixtures only. Run: openagents browser run -- node tools/verify-water-aeration.mjs
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { createServer } from "vite";
const server = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  appType: "custom",
  server: { host: "127.0.0.1", port: 0 },
});
server.middlewares.use((request, response, next) => {
  if (request.url !== "/") return next();
  response.setHeader("content-type", "text/html");
  response.end(
    "<!doctype html><title>Owned water aeration qualification</title>",
  );
});
await server.listen();
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext();
try {
  const page = await context.newPage(),
    pageErrors = [];
  page.on("pageerror", (e) => pageErrors.push(e.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { SourceWaterStreams } =
      await import("/packages/renderer/source-water-streams.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("WebGPU is required.");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    const background = [0.2, 0.4, 0.6, 1];
    const hash = async (bytes) =>
      [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
        .map((x) => x.toString(16).padStart(2, "0"))
        .join("");
    const half = (v) => {
      const sign = v & 0x8000 ? -1 : 1,
        exponent = (v >> 10) & 31,
        mantissa = v & 1023;
      return (
        sign *
        (exponent === 0
          ? (2 ** -14 * mantissa) / 1024
          : exponent === 31
            ? Infinity
            : 2 ** (exponent - 15) * (1 + mantissa / 1024))
      );
    };
    const cases = [],
      count = 32;
    const encodeHalf = (value) => {
      const f = new Float32Array([value]);
      const bits = new Uint32Array(f.buffer)[0];
      const e = ((bits >>> 23) & 255) - 127 + 15;
      return (
        ((bits >>> 16) & 0x8000) |
        (e <= 0 ? 0 : e >= 31 ? 0x7c00 : (e << 10) | ((bits >>> 13) & 1023))
      );
    };
    let maxError = 0;
    async function render({
      time = 0,
      exposure = 1,
      depth = 1,
      normal = [0, 0, -1],
      model = identity,
      mode = "day",
      fog,
      reversed = false,
      sourceNormalRed,
      sourceNormalGreen = 128,
      normalCoverage,
      flipGreenChannel = false,
      refraction = false,
      ior = 2,
      secondLayer = false,
      privateDepthDimensionLimit,
      readPixel = [16, 16],
      vertexNormals,
      checkRefractionAdmission = false,
      uvScale,
      progressUV = 0.5,
      sourceUV = 0.25,
      gradient = false,
      materialIOR = 1.333,
      normalTiling = 1,
      normalStrength = 0.3,
      turbulence = {
        geometryAmplitudeMeters: 0,
        microAmplitudeMeters: 0,
        airScatteringPerMeter: 0,
        airAlbedo: 0.85,
      },
      airModel,
      localLights = [],
      diffuseSH,
      noiseRed,
      noiseValues,

      roughness = 0.4,
      absorption = [0, 0, 0],
      thickness = 0.015,
      environment = true,
      solarRatio = 1,
      constantFog = true,
      gain = [0.01, 0.01, 0.01],
      sun = [0, 0, 0],
      sunDirection = [0, 0, -1],
    } = {}) {
      const vertices = new Float32Array(
        [
          [-1, -1],
          [1, -1],
          [-1, 1],
          [1, 1],
        ].flatMap(([x, y], index) => [
          x,
          y,
          0.5,
          ...(vertexNormals?.[index] ?? normal),
          uvScale === undefined ? sourceUV : (x * 0.5 + 0.5) * uvScale,
          progressUV,
          ...(sourceNormalRed !== undefined ? [1, 0, 0, 1] : []),
        ]),
      ).buffer;
      const indices = new Uint32Array(
        reversed ? [0, 2, 1, 1, 2, 3] : [0, 1, 2, 1, 3, 2],
      ).buffer;
      const noiseReds = noiseValues ?? [noiseRed ?? 64, noiseRed ?? 192];
      const noise = new ArrayBuffer(16 + noiseReds.length * 4),
        header = new DataView(noise);
      header.setUint32(0, 0x41465238, false);
      header.setUint32(4, 1, true);
      header.setUint32(8, noiseReds.length, true);
      header.setUint32(12, 1, true);
      new Uint8Array(noise, 16).set(
        noiseReds.flatMap((red) => [red, 0, 0, 255]),
      );
      const blobs = new Map([
        ["/private-assets/vertices.bin", vertices],
        ["/private-assets/indices.bin", indices],
        ["/private-assets/noise.bin", noise],
      ]);
      let normalMap;
      const normalReds = Array.isArray(sourceNormalRed)
        ? sourceNormalRed
        : [sourceNormalRed];
      if (sourceNormalRed !== undefined) {
        normalMap = new ArrayBuffer(16 + normalReds.length * 4);
        const h = new DataView(normalMap);
        h.setUint32(0, 0x41465238, false);
        h.setUint32(4, 1, true);
        h.setUint32(8, normalReds.length, true);
        h.setUint32(12, 1, true);
        new Uint8Array(normalMap, 16).set(
          normalReds.flatMap((red, index) => [
            red,
            sourceNormalGreen,
            255,
            Array.isArray(normalCoverage)
              ? normalCoverage[index]
              : (normalCoverage ?? 255),
          ]),
        );
        blobs.set("/private-assets/normal.bin", normalMap);
      }
      const descriptor = async (name, data) => ({
        url: "/private-assets/" + name + ".bin",
        sha256: await hash(data),
        byteLength: data.byteLength,
      });
      const recipe = {
        schema: "aethyrforge.source-water-streams.v1",
        vertices: await descriptor("vertices", vertices),
        indices: await descriptor("indices", indices),
        vertexCount: 4,
        ...(normalMap
          ? {
              vertexStride: 48,
              normal: {
                ...(await descriptor("normal", normalMap)),
                width: normalReds.length,
                height: 1,
                colorSpace: "linear",
                encoding: "unreal-normal-rgb",
                flipGreenChannel,
                ...(normalCoverage !== undefined
                  ? { coverage: "bilinear-positive-area-8x8-v1" }
                  : {}),
              },
            }
          : {}),
        indexCount: 6,
        noise: {
          ...(await descriptor("noise", noise)),
          width: noiseReds.length,
          height: 1,
          colorSpace: "linear",
        },
        meshes: [
          { id: "owned", firstIndex: 0, indexCount: 6, center: [0, 0, 0.5] },
        ],
        material: {
          appearanceProfile: "owned-turbulent-stream-v1",
          turbulence: { ...turbulence, ...(airModel ? { airModel } : {}) },
          clearWater: {
            ior: materialIOR,
            roughness,
            normalStrength,
            normalTiling,
            thicknessMeters: thickness,
            absorption,
          },
          flow: [1, 0],
          powerExponent: 0.3,
          fresnelExponent: 1,
          baseReflectFraction: 0.04,
          depthFadeMeters: 0.25,
          refractionMinimum: 1,
        },
        scenarios: {
          day: [
            {
              id: "original",
              meshId: "owned",
              model,
              parameters: [ior, 0.35, 0, 0],
            },
          ],
          night: [],
        },
      };
      if (secondLayer) {
        const farther = [...model];
        farther[14] += 0.1;
        recipe.scenarios.day.push({
          id: "farther",
          meshId: "owned",
          model: farther,
          parameters: [ior, 0.9, 0, 0],
        });
      }
      const effectDevice =
        privateDepthDimensionLimit === undefined
          ? device
          : new Proxy(device, {
              get(target, key) {
                if (key === "limits")
                  return { maxTextureDimension2D: privateDepthDimensionLimit };
                const value = Reflect.get(target, key, target);
                return typeof value === "function" ? value.bind(target) : value;
              },
            });
      const effect = await SourceWaterStreams.create(
        effectDevice,
        recipe,
        async (url) => blobs.get(url),
      );
      // Mutating the caller's object cannot change the installed recipe.
      recipe.material.powerExponent = 10;
      const color = device.createTexture({
        size: [count, count],
        format: "rgba16float",
        sampleCount: 4,
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      });
      const resolved = device.createTexture({
        size: [count, count],
        format: "rgba16float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      });
      const depthTexture = device.createTexture({
        size: [count, count],
        format: "depth24plus",
        sampleCount: 4,
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      });
      const readback = device.createBuffer({
        size: 256 * count,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
      const sourceColor = device.createTexture({
        size: [count, count],
        format: "rgba16float",
        usage:
          GPUTextureUsage.TEXTURE_BINDING |
          GPUTextureUsage.COPY_DST |
          GPUTextureUsage.RENDER_ATTACHMENT,
      });
      const sourcePixels = new Uint16Array(count * count * 4);
      for (let y = 0; y < count; y++)
        for (let x = 0; x < count; x++)
          sourcePixels.set(
            (gradient
              ? [(x + 0.5) / count, (y + 0.5) / count, 0.6, 1]
              : background
            ).map(encodeHalf),
            (y * count + x) * 4,
          );
      device.queue.writeTexture(
        { texture: sourceColor },
        sourcePixels,
        { bytesPerRow: count * 8 },
        [count, count],
      );
      const environmentTexture = device.createTexture({
        size: [1, 1, 2],
        format: "rgba16float",
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      });
      device.queue.writeTexture(
        { texture: environmentTexture },
        new Uint16Array([2, 4, 6, 1, 1, 0.5, 0.25, 1].map(encodeHalf)),
        { bytesPerRow: 8, rowsPerImage: 1 },
        [1, 1, 2],
      );
      const encoder = device.createCommandEncoder();
      const clear = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: color.createView(),
            resolveTarget: resolved.createView(),
            clearValue: background,
            loadOp: "clear",
            storeOp: "store",
          },
        ],
        depthStencilAttachment: {
          view: depthTexture.createView(),
          depthClearValue: depth,
          depthLoadOp: "clear",
          depthStoreOp: "store",
        },
      });
      clear.end();
      const renderFrame = {
        mode,
        time,
        width: count,
        height: count,
        colorView: color.createView(),
        ...(refraction
          ? { refraction: { source: sourceColor, resolve: resolved } }
          : { resolveTarget: resolved.createView() }),
        depthView: depthTexture.createView(),
        viewProjection: identity,
        inverseViewProjection: identity,
        eye: [0, 0, -1],
        effectiveExposure: exposure,
        fog,
        lighting: {
          sunDirectionTowardLight: sunDirection,
          sunRadianceRGB: sun,
          localLights,
          ...(diffuseSH ? { diffuseSH, diffuseRotation: identity } : {}),
        },
        ...(environment
          ? {
              environment: {
                view: environmentTexture.createView({ dimension: "2d-array" }),
                worldToEnvironment: identity,
                mipMax: 0,
                solarRatio,
                hasConstantFog: constantFog,
                gain,
              },
            }
          : {}),
      };
      if (checkRefractionAdmission) {
        for (const invalid of [
          {
            ...renderFrame,
            refraction: { source: sourceColor, resolve: sourceColor },
          },
          { ...renderFrame, refraction: { source: color, resolve: resolved } },
          { ...renderFrame, resolveTarget: resolved.createView() },
        ]) {
          let refused = false;
          try {
            effect.render(encoder, invalid);
          } catch {
            refused = true;
          }
          if (!refused)
            throw new Error(
              "Unsafe source-water texture relationship was accepted.",
            );
        }
        cases.push(
          "refraction rejects feedback, multisampled input and conflicting resolve targets before encoding",
        );
      }
      const stats = effect.render(encoder, renderFrame);
      encoder.copyTextureToBuffer(
        { texture: resolved },
        { buffer: readback, bytesPerRow: 256 },
        [count, count],
      );
      device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);
      const at = (readPixel[1] * 256 + readPixel[0] * 8) / 2,
        data = new Uint16Array(readback.getMappedRange());
      const output = [...data.slice(at, at + 4)].map(half);
      readback.unmap();
      effect.dispose();
      effect.dispose();
      if (effect.bytes !== 0)
        throw new Error("Disposed water retains resource bytes.");
      color.destroy();
      resolved.destroy();
      depthTexture.destroy();
      readback.destroy();
      sourceColor.destroy();
      environmentTexture.destroy();
      return { output, stats };
    }
    function close(actual, expected, label) {
      for (let i = 0; i < 4; i++) {
        const error = Math.abs(actual[i] - expected[i]);
        maxError = Math.max(maxError, error);
        if (!Number.isFinite(actual[i]) || error > 0.001)
          throw new Error(
            `${label}: ${JSON.stringify(actual)} != ${JSON.stringify(expected)}`,
          );
      }
      cases.push(label);
    }
    // Independent exterior-dielectric and microfacet reference, not a fitted image grade.
    const fresnel = (ci, ior) => {
      if (ior === 1) return 0;
      const ct = Math.sqrt(1 - (1 - ci * ci) / (ior * ior));
      return (
        0.5 *
        (((ci - ior * ct) / (ci + ior * ct)) ** 2 +
          ((ior * ci - ct) / (ior * ci + ct)) ** 2)
      );
    };
    const eye = [-1 / 32, 1 / 32, -1.5],
      eyeLength = Math.hypot(...eye),
      view = eye.map((v) => v / eyeLength);
    const dot = (a, b) => a.reduce((v, x, i) => v + x * b[i], 0);
    const env = (ratio = 1, constant = true, gain = [0.01, 0.01, 0.01]) =>
      [2, 4, 6].map(
        (v, i) => (v * ratio + (constant ? [1, 0.5, 0.25][i] : 0)) * gain[i],
      );
    const expected = ({
      ior = 1.333,
      normal = [0, 0, -1],
      reflection = env(),
      absorption = [0, 0, 0],
      thickness = 0.015,
      direct = [0, 0, 0],
    } = {}) => {
      const nv = Math.abs(dot(normal, view)),
        f = fresnel(nv, ior),
        path = thickness / Math.max(nv, 0.1);
      return [
        ...background
          .slice(0, 3)
          .map(
            (v, i) =>
              v * (1 - f) * Math.exp(-absorption[i] * path) +
              reflection[i] * f +
              direct[i],
          ),
        1,
      ];
    };
    close(
      (await render({ materialIOR: 1, refraction: true })).output,
      background,
      "IOR 1 has no dielectric interface or artificial emission",
    );
    close(
      (await render({ refraction: true })).output,
      expected(),
      "actual class resolves dielectric reflection plus frozen-background transmission",
    );
    close(
      (await render()).output,
      expected(),
      "ordinary translucent fallback uses the same environment binding",
    );
    close(
      (await render({ environment: false })).output,
      expected({ reflection: [0, 0, 0] }),
      "missing environment renders finite source-free reflection fallback",
    );
    for (const ratio of [0, 1, 2])
      for (const constant of [false, true])
        close(
          (await render({ solarRatio: ratio, constantFog: constant })).output,
          expected({ reflection: env(ratio, constant) }),
          `solar ${ratio}, constant ${constant}: each layer and storage gain once`,
        );
    close(
      (await render({ gain: [0.02, 0.03, 0.04] })).output,
      expected({ reflection: env(1, true, [0.02, 0.03, 0.04]) }),
      "environment channel gain is applied once",
    );
    const baseline = (await render({ refraction: true })).output;
    for (const exposure of [0.125, 2])
      close(
        (await render({ refraction: true, exposure })).output,
        baseline,
        `owned radiance does not cancel camera exposure ${exposure}`,
      );
    close(
      (await render({ refraction: true, absorption: [0.08, 0.5, 2] })).output,
      expected({ absorption: [0.08, 0.5, 2] }),
      "Beer transmission uses declared physical sheet thickness",
    );
    close(
      (await render({ depth: 0.4, refraction: true })).output,
      background,
      "opaque foreground is never replaced",
    );
    const roughness = 0.4,
      nv = Math.abs(view[2]),
      nh = (1 + nv) / Math.hypot(...view.map((v, i) => v + [0, 0, -1][i])),
      vh = nh;
    const a2 = roughness ** 4,
      D = a2 / (Math.PI * (nh * nh * (a2 - 1) + 1) ** 2);
    const gv = (2 * nv) / (nv + Math.sqrt(a2 + (1 - a2) * nv * nv));
    const unitSpecular = (D * gv * fresnel(vh, 1.333)) / (4 * nv);
    for (const intensity of [25, 50])
      close(
        (await render({ sun: [intensity, intensity, intensity] })).output,
        expected({
          direct: [0, 1, 2].map(() => intensity * 0.01 * unitSpecular),
        }),
        `incident sun ${intensity} uses physical GGX and one storage scale`,
      );
    for (const green of [64, 192]) {
      const result = await render({
        sourceNormalRed: 150,
        sourceNormalGreen: green,
        refraction: true,
      });
      if (!result.output.every(Number.isFinite))
        throw Error("Continuous RGB normal is not finite");
      cases.push(`continuous RGB normal green ${green} remains finite`);
    }
    const detailBase = await render({
      sourceNormalRed: [0, 255, 255, 255],
      normalStrength: 1,
      normalTiling: 1,
    });
    const detailScaled = await render({
      sourceNormalRed: [0, 255, 255, 255],
      normalStrength: 1,
      normalTiling: 2,
    });
    if (
      Math.max(
        ...detailBase.output.map((v, i) =>
          Math.abs(v - detailScaled.output[i]),
        ),
      ) < 0.0005
    )
      throw Error(
        "Owned normal detail frequency did not change the sampled optical response",
      );
    cases.push(
      "declared detail tiling changes continuous normal sampling without scaling flow speed",
    );
    const point = {
      id: "qa",
      label: "qa",
      kind: "point",
      position: [0, 0, -0.5],
      direction: [0, 0, 1],
      color: [1, 0.5, 0.25],
      intensity: 100,
      intensityUnits: "candelas",
      inverseSquaredFalloff: true,
      range: 10,
      castShadows: true,
      enabled: true,
      temperatureKelvin: 6500,
      useTemperature: false,
      raw: {},
    };
    const localReference = (light) => {
      const delta = [-1 / 32, 1 / 32, -1],
        dist = Math.hypot(...delta),
        L = delta.map((v) => v / dist),
        nv = -view[2],
        nl = -L[2];
      const H = view.map((v, i) => v + L[i]),
        hl = Math.hypot(...H),
        nh = -H[2] / hl,
        vh = dot(view, H) / hl;
      const a2 = 0.4 ** 4,
        D = a2 / (Math.PI * (nh * nh * (a2 - 1) + 1) ** 2),
        gv = (2 * nv) / (nv + Math.sqrt(a2 + (1 - a2) * nv * nv)),
        gl = (2 * nl) / (nl + Math.sqrt(a2 + (1 - a2) * nl * nl));
      const attenuation =
        light.inverseSquaredFalloff === false
          ? (1 - (dist / 10) ** 2) ** 4
          : (1 - (dist / 10) ** 4) ** 2 / (dist * dist + 0.0001);
      const specular = (D * gv * gl * fresnel(vh, 1.333)) / (4 * nv);
      return light.color.map((c) => c * 100 * 0.01 * attenuation * specular);
    };
    for (const light of [
      point,
      {
        ...point,
        kind: "spot",
        innerConeAngleRadians: 0.2,
        outerConeAngleRadians: 0.4,
      },
      {
        ...point,
        inverseSquaredFalloff: false,
        falloffExponent: 4,
        intensityUnits: "unitless",
      },
    ])
      close(
        (await render({ localLights: [light] })).output,
        expected({ direct: localReference(light) }),
        `source ${light.kind} ${light.inverseSquaredFalloff} radiometry illuminates water`,
      );
    close(
      (
        await render({
          localLights: [
            {
              ...point,
              kind: "spot",
              direction: [0, 0, -1],
              innerConeAngleRadians: 0.2,
              outerConeAngleRadians: 0.4,
            },
          ],
        })
      ).output,
      expected(),
      "spotlight cone rejects water behind its emission direction",
    );
    const sh = new Array(36).fill(0);
    for (let c = 0; c < 3; c++) sh[c] = [1, 2, 3][c] / 0.2820947918;
    const t = {
      geometryAmplitudeMeters: 0,
      microAmplitudeMeters: 0,
      airScatteringPerMeter: 20,
      airAlbedo: 0.85,
    };
    const fraction = (0.5 - 0.15) / (0.95 - 0.15),
      progress = fraction * fraction * (3 - 2 * fraction),
      airNv = -view[2],
      air = 1 - Math.exp((-0.015 / airNv) * progress * 20);
    const water = expected(),
      mixed = water.map((v, i) =>
        i === 3 ? 1 : v * (1 - air) + [1, 2, 3][i] * 0.01 * 0.85 * air,
      );
    close(
      (
        await render({
          refraction: true,
          turbulence: t,
          noiseRed: 255,
          diffuseSH: sh,
        })
      ).output,
      mixed,
      "air optical depth scatters source SH exactly once without emission or exposure cancellation",
    );
    for (const time of [0, 3, 10]) {
      const r = await render({
        time,
        refraction: true,
        turbulence: {
          ...t,
          geometryAmplitudeMeters: 0.004,
          microAmplitudeMeters: 0.0015,
        },
      });
      if (!r.output.every(Number.isFinite))
        throw Error("Deformed turbulent water is not finite");
      cases.push(`actual deformed nearest-interface render time ${time}`);
    }
    const hg = (cosine, g) =>
      (1 - g * g) / (4 * Math.PI * (1 + g * g + 2 * g * cosine) ** 1.5);
    const airModel = {
      kind: "henyey-greenstein-v1",
      nozzleFraction: 0.2,
      asymmetry: 0.45,
    };
    const aeratedFraction =
      1 - Math.exp((-0.015 / -view[2]) * (0.2 + 0.8 * progress) * 20);
    const medium = {
      geometryAmplitudeMeters: 0,
      microAmplitudeMeters: 0,
      airScatteringPerMeter: 20,
      airAlbedo: 0.85,
    };
    const volumeMix = (water, source, air = aeratedFraction) =>
      water.map((v, i) => (i === 3 ? 1 : v * (1 - air) + source[i] * air));
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel,
          noiseRed: 255,
          diffuseSH: sh,
        })
      ).output,
      volumeMix(expected(), [0.0085, 0.017, 0.0255]),
      "declared nozzle air preserves constant sky radiance while increasing only optical coverage",
    );
    for (const g of [0, 0.45, 0.8]) {
      const model = { ...airModel, asymmetry: g };
      const radiance = [25, 15, 5].map((v) => v * 0.01 * hg(view[2], g) * 0.85);
      close(
        (
          await render({
            refraction: true,
            turbulence: medium,
            airModel: model,
            noiseRed: 255,
            sun: [25, 15, 5],
            sunDirection: [0, 0, 1],
          })
        ).output,
        volumeMix(expected(), radiance),
        `backlit solar HG scattering g=${g} reaches the viewer without surface-normal clipping`,
      );
    }
    const behind = { ...point, position: [0, 0, 1.5] };
    const backDelta = [-1 / 32, 1 / 32, 1],
      backDist = Math.hypot(...backDelta),
      backL = backDelta.map((v) => v / backDist),
      attenuation =
        (1 - (backDist / 10) ** 4) ** 2 / (backDist * backDist + 0.0001);
    const backRadiance = behind.color.map(
      (c) => c * 100 * 0.01 * attenuation * hg(dot(backL, view), 0.45) * 0.85,
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel,
          noiseRed: 255,
          localLights: [behind],
        })
      ).output,
      volumeMix(expected(), backRadiance),
      "backlit physical point source scatters through air with source attenuation and one storage scale",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: { ...medium, airScatteringPerMeter: 0 },
          airModel,
          noiseRed: 255,
          sun: [0, 0, 0],
        })
      ).output,
      expected(),
      "zero air optical depth preserves clear response despite an explicit phase model",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel,
          noiseRed: 255,
          depth: 0.4,
          sun: [25, 25, 25],
        })
      ).output,
      background,
      "camera-opaque foreground occludes aerated water",
    );
    const nozzle = { kind: "diffuse-foam-v1", nozzleFraction: 0.2 };
    for (const progressUV of [0, 0.5, 1]) {
      const original = (
        await render({
          refraction: true,
          turbulence: medium,
          noiseRed: 255,
          diffuseSH: sh,
          progressUV,
        })
      ).output;
      close(
        (
          await render({
            refraction: true,
            turbulence: medium,
            airModel: { ...nozzle, nozzleFraction: 0 },
            noiseRed: 255,
            diffuseSH: sh,
            progressUV,
          })
        ).output,
        original,
        `explicit zero-nozzle Lambert preserves omitted profile at progress ${progressUV}`,
      );
      const x = Math.min(1, Math.max(0, (progressUV - 0.15) / 0.8)),
        baseCoverage = x * x * (3 - 2 * x);
      const coverage = 0.2 + 0.8 * baseCoverage,
        fraction = 1 - Math.exp((-0.015 / -view[2]) * coverage * 20);
      close(
        (
          await render({
            refraction: true,
            turbulence: medium,
            airModel: nozzle,
            noiseRed: 255,
            diffuseSH: sh,
            progressUV,
          })
        ).output,
        volumeMix(expected(), [0.0085, 0.017, 0.0255], fraction),
        `nozzle-only Lambert changes coverage without sky phase at progress ${progressUV}`,
      );
    }
    for (const sunDirection of [
      [0, 0, -1],
      [0, 0, 1],
    ]) {
      const direct =
        sunDirection[2] < 0
          ? [25, 15, 5].map((v) => v * 0.01 * unitSpecular)
          : [0, 0, 0];
      const source =
        sunDirection[2] < 0
          ? [25, 15, 5].map((v) => ((v * 0.01) / Math.PI) * 0.85)
          : [0, 0, 0];
      close(
        (
          await render({
            refraction: true,
            turbulence: medium,
            airModel: nozzle,
            noiseRed: 255,
            sun: [25, 15, 5],
            sunDirection,
          })
        ).output,
        volumeMix(expected({ direct }), source),
        `nozzle-only solar retains Lambert normal clipping for ${sunDirection[2]}`,
      );
    }
    const frontDistance = Math.hypot(-1 / 32, 1 / 32, -1),
      frontNl = 1 / frontDistance;
    const frontAttenuation =
      (1 - (frontDistance / 10) ** 4) ** 2 / (frontDistance ** 2 + 0.0001);
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: nozzle,
          noiseRed: 255,
          localLights: [point],
        })
      ).output,
      volumeMix(
        expected({ direct: localReference(point) }),
        point.color.map(
          (c) =>
            ((c * 100 * 0.01 * frontAttenuation * frontNl) / Math.PI) * 0.85,
        ),
      ),
      "nozzle-only physical point light retains Lambert energy and one storage gain",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: nozzle,
          noiseRed: 255,
          localLights: [behind],
        })
      ).output,
      volumeMix(expected(), [0, 0, 0]),
      "nozzle-only backlit point does not enable HG scattering",
    );
    const sourceEnergyModel = {
      kind: "source-energy-foam-v1",
      nozzleFraction: 0.2,
    };
    const sourceIncident = (noise, exposure = 1) =>
      new Array(3).fill(((noise ** 0.3 * 0.01) / exposure) * 0.85);
    for (const exposure of [0.125, 1, 2]) {
      close(
        (
          await render({
            refraction: true,
            turbulence: medium,
            airModel: sourceEnergyModel,
            noiseValues: [255, 64, 0, 255],
            exposure,
          })
        ).output,
        volumeMix(expected(), sourceIncident(159.5 / 255, exposure)),
        `source-energy foam uses source1xnoise power and exposure ${exposure} exactly once`,
      );
    }
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: sourceEnergyModel,
          noiseValues: [255, 64, 0, 255],
          time: 0.125,
        })
      ).output,
      volumeMix(expected(), sourceIncident(64 / 255)),
      "source-energy flow advects independently of unchanged8xcoverage",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: sourceEnergyModel,
          noiseValues: [255, 64, 0, 255],
          sourceUV: 0.625,
        })
      ).output,
      volumeMix(expected(), [0, 0, 0]),
      "zero source-energy noise has no invented emission floor",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: sourceEnergyModel,
          noiseRed: 255,
          sun: [25, 15, 5],
        })
      ).output,
      volumeMix(
        expected({ direct: [25, 15, 5].map((v) => v * 0.01 * unitSpecular) }),
        sourceIncident(1),
      ),
      "source-energy foam preserves directional GGX without adding diffuse-light gain",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: sourceEnergyModel,
          noiseRed: 255,
          diffuseSH: sh,
        })
      ).output,
      volumeMix(expected(), sourceIncident(1)),
      "source-energy foam replaces only the diffuse irradiance term",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: sourceEnergyModel,
          noiseRed: 255,
          localLights: [point],
        })
      ).output,
      volumeMix(expected({ direct: localReference(point) }), sourceIncident(1)),
      "source-energy foam keeps source local specular and attenuation",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: { ...medium, airScatteringPerMeter: 0 },
          airModel: sourceEnergyModel,
          noiseRed: 255,
        })
      ).output,
      expected(),
      "zero air exactly retains clear water even with source-energy foam",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: sourceEnergyModel,
          noiseRed: 255,
          depth: 0.4,
        })
      ).output,
      background,
      "opaque foreground occludes source-energy foam",
    );
    const fogT = Math.exp(-0.1 * Math.hypot(1 / 32, 1 / 32, 1.5)),
      fogColor = [0.01, 0.02, 0.03],
      f = fresnel(-view[2], 1.333);
    const fogBase = background.map((v, i) =>
      i === 3
        ? 1
        : v * (1 - f) + env()[i] * f * fogT + fogColor[i] * (1 - fogT) * f,
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: sourceEnergyModel,
          noiseRed: 255,
          fog: { density: 0.1, heightRate: 0, color: fogColor },
        })
      ).output,
      volumeMix(
        fogBase,
        sourceIncident(1).map((v, i) => v * fogT + fogColor[i] * (1 - fogT)),
      ),
      "source-energy foam shares analytic fog and one stored radiance scale",
    );
    close(
      (
        await render({
          turbulence: medium,
          airModel: sourceEnergyModel,
          noiseRed: 255,
        })
      ).output,
      volumeMix(expected(), sourceIncident(1)),
      "source-energy foam retains ordinary blend fallback",
    );
    const additiveModel = {
      kind: "source-energy-foam-additive-v1",
      nozzleFraction: 0.2,
    };
    const sumIncident = (a, b) => a.map((v, i) => v + b[i]);
    const diffuseIncident = [0.0085, 0.017, 0.0255];
    for (const exposure of [0.125, 1, 2]) {
      close(
        (
          await render({
            refraction: true,
            turbulence: medium,
            airModel: additiveModel,
            noiseValues: [255, 64, 0, 255],
            diffuseSH: sh,
            exposure,
          })
        ).output,
        volumeMix(
          expected(),
          sumIncident(diffuseIncident, sourceIncident(159.5 / 255, exposure)),
        ),
        `additive foam retains physical sky and adds source energy with one exposure cancellation ${exposure}`,
      );
    }
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: additiveModel,
          noiseRed: 255,
          sun: [25, 15, 5],
        })
      ).output,
      volumeMix(
        expected({ direct: [25, 15, 5].map((v) => v * 0.01 * unitSpecular) }),
        sumIncident(
          [25, 15, 5].map((v) => ((v * 0.01) / Math.PI) * 0.85),
          sourceIncident(1),
        ),
      ),
      "additive foam preserves physical directional diffuse and GGX while adding source energy",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: additiveModel,
          noiseRed: 255,
          localLights: [point],
        })
      ).output,
      volumeMix(
        expected({ direct: localReference(point) }),
        sumIncident(
          point.color.map(
            (c) =>
              ((c * 100 * 0.01 * frontAttenuation * frontNl) / Math.PI) * 0.85,
          ),
          sourceIncident(1),
        ),
      ),
      "additive foam preserves physical local diffuse, attenuation and specular",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: additiveModel,
          noiseValues: [255, 64, 0, 255],
          sourceUV: 0.625,
          diffuseSH: sh,
        })
      ).output,
      volumeMix(expected(), diffuseIncident),
      "zero source noise retains physical foam without an emission floor",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: { ...medium, airScatteringPerMeter: 0 },
          airModel: additiveModel,
          noiseRed: 255,
          diffuseSH: sh,
        })
      ).output,
      expected(),
      "zero air retains clear water with the additive model",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: additiveModel,
          noiseRed: 255,
          depth: 0.4,
        })
      ).output,
      background,
      "opaque foreground occludes additive foam",
    );
    close(
      (
        await render({
          refraction: true,
          turbulence: medium,
          airModel: additiveModel,
          noiseRed: 255,
          diffuseSH: sh,
          fog: { density: 0.1, heightRate: 0, color: fogColor },
        })
      ).output,
      volumeMix(
        fogBase,
        sumIncident(diffuseIncident, sourceIncident(1)).map(
          (v, i) => v * fogT + fogColor[i] * (1 - fogT),
        ),
      ),
      "physical and source foam share fog without adding a second fog fill",
    );
    close(
      (
        await render({
          turbulence: medium,
          airModel: additiveModel,
          noiseRed: 255,
          diffuseSH: sh,
        })
      ).output,
      volumeMix(expected(), sumIncident(diffuseIncident, sourceIncident(1))),
      "additive foam retains ordinary blend fallback",
    );
    const mirrored = [...identity];
    mirrored[0] = -1;
    close(
      (await render({ model: mirrored })).output,
      expected(),
      "mirrored geometric basis retains dielectric response",
    );
    const fallback = await render({
      refraction: true,
      privateDepthDimensionLimit: 16,
    });
    if (fallback.stats.refraction) throw Error("Private depth cap ignored");
    close(
      fallback.output,
      expected(),
      "bounded refraction fallback still resolves clear reflection",
    );
    const { WGSL_WATER_TURBULENCE, waterSurfaceField } =
      await import("/packages/renderer/water-turbulence.ts");
    const fieldQueries = [];
    for (const time of [0, 3, 10, 31536000])
      for (const micro of [false, true]) fieldQueries.push({ time, micro });
    const fieldValues = new Float32Array(fieldQueries.length * 8);
    fieldQueries.forEach(({ time, micro }, i) =>
      fieldValues.set(
        [0.13, 2.16, -0.83, time, micro ? 0.0015 : 0.004, micro ? 1 : 0, 0, 0],
        i * 8,
      ),
    );
    const fieldInput = device.createBuffer({
      size: fieldValues.byteLength,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(fieldInput, 0, fieldValues);
    const fieldOutput = device.createBuffer({
      size: fieldQueries.length * 16,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
    });
    const fieldRead = device.createBuffer({
      size: fieldQueries.length * 16,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    const fieldShader = device.createShaderModule({
      code:
        WGSL_WATER_TURBULENCE +
        `struct Q{positionTime:vec4<f32>,params:vec4<f32>};@group(0)@binding(0)var<storage,read>q:array<Q>;@group(0)@binding(1)var<storage,read_write>r:array<vec4<f32>>;@compute@workgroup_size(1)fn main(@builtin(global_invocation_id)id:vec3<u32>){let a=q[id.x];if a.params.y>.5{r[id.x]=waterMicroField(a.positionTime.xyz,a.positionTime.w,a.params.x);}else{r[id.x]=waterGeometryField(a.positionTime.xyz,a.positionTime.w,a.params.x);}}`,
    });
    const fieldPipeline = await device.createComputePipelineAsync({
      layout: "auto",
      compute: { module: fieldShader, entryPoint: "main" },
    });
    const fieldGroup = device.createBindGroup({
      layout: fieldPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: fieldInput } },
        { binding: 1, resource: { buffer: fieldOutput } },
      ],
    });
    const fieldEncoder = device.createCommandEncoder(),
      fieldPass = fieldEncoder.beginComputePass();
    fieldPass.setPipeline(fieldPipeline);
    fieldPass.setBindGroup(0, fieldGroup);
    fieldPass.dispatchWorkgroups(fieldQueries.length);
    fieldPass.end();
    fieldEncoder.copyBufferToBuffer(
      fieldOutput,
      0,
      fieldRead,
      0,
      fieldQueries.length * 16,
    );
    device.queue.submit([fieldEncoder.finish()]);
    await fieldRead.mapAsync(GPUMapMode.READ);
    const fieldResult = new Float32Array(fieldRead.getMappedRange());
    let maxFieldError = 0;
    for (let i = 0; i < fieldQueries.length; i++) {
      const { time, micro } = fieldQueries[i],
        q = fieldValues.subarray(i * 8, i * 8 + 3),
        expected = waterSurfaceField(
          [...q],
          time,
          micro ? 0.0015 : 0.004,
          micro,
        ),
        values = [expected.height, ...expected.gradient];
      for (let c = 0; c < 4; c++) {
        const error = Math.abs(fieldResult[i * 4 + c] - values[c]);
        maxFieldError = Math.max(error, maxFieldError);
        if (!Number.isFinite(fieldResult[i * 4 + c]) || error > 0.0001)
          throw Error(`World field CPU/GPU mismatch ${i}/${c}: ${error}`);
      }
    }
    fieldRead.unmap();
    fieldInput.destroy();
    fieldOutput.destroy();
    fieldRead.destroy();
    cases.push(
      "analytic height/normal-gradient GPU field agrees at fixed times through one year",
    );
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return { cases, maxError, maxFieldError, errors };
  });
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  console.log(JSON.stringify(result));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
