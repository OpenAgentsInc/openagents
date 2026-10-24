// Original fixtures only. Run: openagents browser run -- node tools/verify-clear-water.mjs
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
  response.end("<!doctype html><title>Owned clear water qualification</title>");
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
      gradient = false,
      materialIOR = 1.333,
      normalTiling = 1,
      normalStrength = 0.3,
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
          uvScale === undefined ? 0.25 : (x * 0.5 + 0.5) * uvScale,
          0.5,
          ...(sourceNormalRed !== undefined ? [1, 0, 0, 1] : []),
        ]),
      ).buffer;
      const indices = new Uint32Array(
        reversed ? [0, 2, 1, 1, 2, 3] : [0, 1, 2, 1, 3, 2],
      ).buffer;
      const noise = new ArrayBuffer(24),
        header = new DataView(noise);
      header.setUint32(0, 0x41465238, false);
      header.setUint32(4, 1, true);
      header.setUint32(8, 2, true);
      header.setUint32(12, 1, true);
      new Uint8Array(noise, 16).set([64, 0, 0, 255, 192, 0, 0, 255]);
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
          width: 2,
          height: 1,
          colorSpace: "linear",
        },
        meshes: [
          { id: "owned", firstIndex: 0, indexCount: 6, center: [0, 0, 0.5] },
        ],
        material: {
          appearanceProfile: "owned-clear-stream-v1",
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
    for (const normal of [
      [0.6, 0, -0.8],
      [0.995, 0, -Math.sqrt(1 - 0.995 ** 2)],
    ])
      close(
        (await render({ normal })).output,
        expected({ normal }),
        "finite exact dielectric Fresnel at oblique/grazing view",
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
    // Independently trace a parallel slab, intersecting the exit ray with a
    // frontoparallel background. Physical displacement must not grow with the gap.
    const normal = [0.6, 0, -0.8],
      incident = view.map((x) => -x),
      ci = -dot(normal, incident),
      eta = 1 / 1.333;
    const ct = Math.sqrt(1 - eta * eta * (1 - ci * ci));
    const inside = incident.map(
      (v, i) => eta * v + (eta * ci - ct) * normal[i],
    );
    for (const thickness of [0.01, 0.1])
      for (const depth of [0.8, 0.95]) {
        const delta = inside.map(
          (v, i) => thickness * (v / ct - incident[i] / ci),
        );
        const shifted = delta.map(
          (v, i) => v - (incident[i] * delta[2]) / incident[2],
        );
        const refracted = [
          (1 / 32 + shifted[0]) * 0.5 + 0.5,
          -(-1 / 32 + shifted[1]) * 0.5 + 0.5,
          0.6,
        ];
        const f = fresnel(ci, 1.333),
          expected = [...refracted.map((v) => v * (1 - f)), 1];
        close(
          (
            await render({
              normal,
              thickness,
              depth,
              refraction: true,
              environment: false,
              gradient: true,
            })
          ).output,
          expected,
          `parallel sheet ${thickness}m gap ${depth - 0.5}m has bounded physical displacement`,
        );
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
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return { cases, maxError, errors };
  });
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  console.log(JSON.stringify(result));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
