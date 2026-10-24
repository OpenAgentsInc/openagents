// Original fixtures only. Run: openagents browser run -- node tools/verify-source-water.mjs
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
    "<!doctype html><title>Owned source water qualification</title>",
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
      hybrid = false,
      hybridSettings = {
        normalStrength: 0,
        normalTiling: 1,
        geometryAmplitudeMeters: 0,
        microAmplitudeMeters: 0,
      },
      flow = [1, 0],
      lighting,
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
                encoding: hybrid
                  ? "unreal-normal-rgb"
                  : "unreal-normal-red-splat",
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
          ...(hybrid
            ? {
                appearanceProfile: "owned-source-emissive-stream-v1",
                emissiveWater: hybridSettings,
                refractionMode: "source-screen-displacement-v1",
              }
            : {}),
          flow,
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
            [(x + 0.5) / count, (y + 0.5) / count, 0.6, 1].map(encodeHalf),
            (y * count + x) * 4,
          );
      device.queue.writeTexture(
        { texture: sourceColor },
        sourcePixels,
        { bytesPerRow: count * 8 },
        [count, count],
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
        ...(lighting ? { lighting } : {}),
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
    const reference = ({
      opacity = 0.35,
      noise = 64 / 255,
      exposure = 1,
      nz = 1,
      fade = 1,
      transmittance = 1,
      fill = 0,
      fogColor = [0, 0, 0],
    } = {}) => {
      const rayLength = Math.hypot(1 / 32, 1 / 32, 1.5);
      const alpha =
        opacity * (0.04 + 0.96 * (1 - (nz * 1.5) / rayLength)) * fade;
      const emission = (noise ** 0.3 * 0.01) / exposure;
      return background.map((v, i) =>
        i < 3
          ? v * (1 - alpha) +
            (emission * transmittance + fogColor[i] * fill) * alpha
          : 1,
      );
    };
    const baseline = await render();
    close(
      baseline.output,
      reference(),
      "source power, Fresnel opacity, premultiplied blend and MSAA resolve",
    );
    if (baseline.stats.instances !== 1 || baseline.stats.triangles !== 2)
      throw new Error("Incorrect instance or triangle count.");
    close(
      (await render({ exposure: 2 })).output,
      reference({ exposure: 2 }),
      "source EyeAdaptation compensation and one storage scale",
    );
    close(
      (await render({ time: 0.5 })).output,
      reference({ noise: 192 / 255 }),
      "fixed-time source UV flow",
    );
    close(
      (await render({ depth: 0.4 })).output,
      background,
      "opaque depth occlusion",
    );
    close(
      (await render({ depth: 0.625 })).output,
      reference({ fade: 0.5 }),
      "source 25cm linear-view depth fade",
    );
    close(
      (await render({ mode: "night" })).output,
      background,
      "separate scenario placements",
    );
    close(
      (await render({ normal: [0, 0, 1], reversed: true })).output,
      baseline.output,
      "two-sided source mesh",
    );
    const reflected = [...identity];
    reflected[0] = -1;
    close(
      (await render({ model: reflected })).output,
      baseline.output,
      "physical backface under mirrored model",
    );
    const normalReference = (
      mapped,
      noise = 64 / 255,
      exposure = 1,
      sourceBackground = background,
    ) => {
      const eye = [-1 / 32, 1 / 32, -1.5],
        d = Math.hypot(...eye),
        l = Math.hypot(...mapped);
      const dot = Math.max(
        0,
        Math.min(1, mapped.reduce((sum, x, i) => sum + x * eye[i], 0) / d / l),
      );
      const alpha = 0.35 * (0.04 + 0.96 * (1 - dot));
      const emission = (noise ** 0.3 * 0.01) / exposure;
      return sourceBackground.map((v, i) =>
        i < 3 ? v * (1 - alpha) + emission * alpha : 1,
      );
    };
    const hybridBase = { hybrid: true, sourceNormalRed: 128 };
    for (const exposure of [0.125, 1, 2]) {
      close(
        (await render({ ...hybridBase, exposure })).output,
        normalReference([0, 0, -1], 64 / 255, exposure),
        `hybrid source power/Fresnel/exposure compensation at ${exposure}`,
      );
    }
    for (const red of [64, 192]) {
      close(
        (
          await render({
            ...hybridBase,
            sourceNormalRed: red,
            sourceNormalGreen: 192,
            hybridSettings: {
              normalStrength: 0.3,
              normalTiling: 1,
              geometryAmplitudeMeters: 0,
              microAmplitudeMeters: 0,
            },
          })
        ).output,
        normalReference([
          ((red / 255) * 2 - 1) * 0.3,
          ((192 / 255) * 2 - 1) * 0.3,
          -1,
        ]),
        `hybrid continuous RGB normal ${red} retains source opacity without scalar sign collapse`,
      );
    }
    for (const time of [3, 3.125, 86400]) {
      const phase = (((time * 0.17) % 1) + 1) % 1,
        at = ((0.25 + phase) % 1) * 2 - 0.5,
        index = Math.floor(at),
        t = at - index;
      const colors = [64, 192],
        noise =
          (colors[((index % 2) + 2) % 2] * (1 - t) +
            colors[(((index + 1) % 2) + 2) % 2] * t) /
          255;
      close(
        (await render({ ...hybridBase, time, flow: [0.17, -1.33] })).output,
        normalReference([0, 0, -1], noise),
        `hybrid source flow remains bounded and reversible at time ${time}`,
      );
    }
    close(
      (
        await render({
          ...hybridBase,
          lighting: {
            sunDirectionTowardLight: [0, 0, -1],
            sunRadianceRGB: [100, 200, 300],
          },
        })
      ).output,
      normalReference([0, 0, -1]),
      "hybrid source emission has no extra direct-light intensity multiplier",
    );
    close(
      (await render({ ...hybridBase, depth: 0.4 })).output,
      background,
      "hybrid source energy respects opaque foreground",
    );
    close(
      (await render({ ...hybridBase, depth: 0.625 })).output,
      reference({ fade: 0.5 }),
      "hybrid retains source depth fade",
    );
    close(
      (await render({ ...hybridBase, refraction: true })).output,
      normalReference([0, 0, -1], 64 / 255, 1, [16.5 / 32, 16.5 / 32, 0.6, 1]),
      "hybrid nearest-interface composition uses one frozen background and source alpha once",
    );
    for (const time of [0, 3, 10]) {
      const result = await render({
        ...hybridBase,
        time,
        refraction: true,
        hybridSettings: {
          normalStrength: 0.3,
          normalTiling: 1,
          geometryAmplitudeMeters: 0.004,
          microAmplitudeMeters: 0.0015,
        },
      });
      if (!result.output.every(Number.isFinite))
        throw Error("Hybrid deformed source energy is not finite");
      cases.push(`hybrid identical depth/color deformation at time ${time}`);
    }
    const signedNormal = await render({
      sourceNormalRed: 255,
      normal: [0, 0, 1],
    });
    close(
      signedNormal.output,
      normalReference([1, -1, 1]),
      "source signed normal red splat converts final tangent Y",
    );
    close(
      (await render({ sourceNormalRed: 0, normal: [0, 0, 1] })).output,
      normalReference([-1, 1, -1]),
      "negative signed source normal red flips all components",
    );
    close(
      (
        await render({
          sourceNormalRed: 255,
          sourceNormalGreen: 0,
          flipGreenChannel: true,
          normal: [0, 0, 1],
        })
      ).output,
      signedNormal.output,
      "source green inversion cannot alter extracted red scalar",
    );
    const combine = (negative, positive, coverage) =>
      negative.map(
        (value, index) => value * (1 - coverage) + positive[index] * coverage,
      );
    for (const coverage of [0, 128, 255]) {
      close(
        (
          await render({
            sourceNormalRed: 0,
            normal: [0, 0, 1],
            normalCoverage: coverage,
          })
        ).output,
        combine(
          normalReference([-1, 1, -1]),
          normalReference([1, -1, 1]),
          coverage / 255,
        ),
        `coverage Fresnel evaluates both exact source polarities at${coverage}/255`,
      );
    }
    close(
      (
        await render({
          sourceNormalRed: [0, 255, 0, 255, 0, 255, 0, 255],
          normal: [0, 0, 1],
          normalCoverage: [0, 255, 0, 255, 0, 255, 0, 255],
          uvScale: 32,
        })
      ).output,
      combine(
        normalReference([-1, 1, -1], 128 / 255),
        normalReference([1, -1, 1], 128 / 255),
        128 / 255,
      ),
      "coverage mip preserves mixed signs instead of thresholding averaged red",
    );
    for (const [name, options] of [
      ["backface", { reversed: true }],
      ["mirrored", { model: reflected }],
      ["refraction", { refraction: true, depth: 0.9 }],
    ]) {
      const shared = { ...options, normal: [0, 0, 1] };
      const negative = (await render({ ...shared, sourceNormalRed: 0 })).output,
        positive = (await render({ ...shared, sourceNormalRed: 255 })).output;
      close(
        (await render({ ...shared, sourceNormalRed: 0, normalCoverage: 128 }))
          .output,
        combine(negative, positive, 128 / 255),
        `coverage weights remain normalized through${name}`,
      );
    }
    close(
      (await render({ sourceNormalRed: [127, 128], time: 0.25 })).output,
      (await render({ time: 0.25 })).output,
      "exact bilinear zero signed-red normal uses the geometric normal",
    );
    const centeredModel = [...identity];
    centeredModel[12] = 1 / 32;
    close(
      (
        await render({
          model: centeredModel,
          vertexNormals: [
            [0, 0, 1],
            [0, 0, -1],
            [0, 0, 1],
            [0, 0, -1],
          ],
        })
      ).output,
      (await render({ model: centeredModel, normal: [0, 0, 1] })).output,
      "cancelled interpolated N and T use a finite source-facing triangle frame",
    );
    const refractedReference = (normal, ior) => {
      const world = [1 / 32, -1 / 32, 0.5],
        incident = [...world];
      incident[2] += 1;
      const d = Math.hypot(...incident);
      for (let i = 0; i < 3; i++) incident[i] /= d;
      let n = normal.map((x) => -x / Math.hypot(...normal));
      if (n.reduce((s, x, i) => s + x * incident[i], 0) > 0)
        n = n.map((x) => -x);
      const eta = 1 / ior,
        c = -n.reduce((s, x, i) => s + x * incident[i], 0),
        k = 1 - eta * eta * (1 - c * c);
      const ray = incident.map(
        (x, i) => eta * x + (eta * c - Math.sqrt(k)) * n[i],
      );
      const point = world.map((x, i) => x + (ray[i] * (0.9 - 0.5)) / ray[2]);
      const uv = [0.5 + 0.5 * point[0], 0.5 - 0.5 * point[1], 0.6];
      if (ior === 1) {
        uv[0] = 16.5 / 32;
        uv[1] = 16.5 / 32;
      }
      const mapped = normal,
        eye = [-world[0], -world[1], -1.5];
      const cosine = Math.max(
        0,
        Math.min(
          1,
          mapped.reduce((s, x, i) => s + x * eye[i], 0) /
            Math.hypot(...mapped) /
            Math.hypot(...eye),
        ),
      );
      const alpha = 0.35 * (0.04 + 0.96 * (1 - cosine)),
        emission = (64 / 255) ** 0.3 * 0.01;
      return [...uv.map((x) => x * (1 - alpha) + emission * alpha), 1];
    };
    const slanted = [0.8, 0, -0.6];
    const identityRefraction = await render({
      refraction: true,
      depth: 0.9,
      normal: slanted,
      ior: 1,
      checkRefractionAdmission: true,
    });
    if (!identityRefraction.stats.refraction)
      throw new Error("Refraction did not activate.");
    close(
      identityRefraction.output,
      refractedReference(slanted, 1),
      "IOR one preserves the original frozen background pixel",
    );
    const refracted = await render({
      refraction: true,
      depth: 0.9,
      normal: slanted,
    });
    close(
      refracted.output,
      refractedReference(slanted, 1 + 64 / 255),
      "source noise-controlled IOR samples Snell plane intersection",
    );
    if (Math.abs(refracted.output[0] - identityRefraction.output[0]) < 0.01)
      throw new Error("Refraction did not move the background.");
    close(
      (
        await render({
          refraction: true,
          depth: 0.9,
          normal: slanted,
          secondLayer: true,
        })
      ).output,
      refracted.output,
      "nearest interface replaces one frozen background without double blending",
    );
    close(
      (await render({ refraction: true, depth: 0.4 })).output,
      background,
      "nearest-stream pass still rejects opaque foreground",
    );
    close(
      (await render({ refraction: true, depth: 1, normal: slanted })).output,
      identityRefraction.output,
      "missing background depth retains the original frozen pixel",
    );
    const edge = {
      refraction: true,
      depth: 0.99,
      normal: [-1, 0, -0.001],
      time: 0.5,
      readPixel: [30, 16],
    };
    close(
      (await render({ ...edge, ior: 4 })).output,
      (await render({ ...edge, ior: 1 })).output,
      "offscreen refracted ray retains the original frozen pixel",
    );
    const boundedFallback = await render({
      refraction: true,
      privateDepthDimensionLimit: 16,
    });
    if (boundedFallback.stats.refraction)
      throw new Error("Private depth limit was ignored.");
    close(
      boundedFallback.output,
      baseline.output,
      "private depth limit resolves ordinary source transparency",
    );
    const rayLength = Math.hypot(1 / 32, 1 / 32, 1.5);
    close(
      (
        await render({
          fog: { density: 0.5, heightRate: 0, color: [0.01, 0.02, 0.03] },
        })
      ).output,
      reference({
        transmittance: Math.exp(-0.5 * rayLength),
        fill: 1 - Math.exp(-0.5 * rayLength),
        fogColor: [0.01, 0.02, 0.03],
      }),
      "analytic height fog composition",
    );
    close(
      (
        await render({
          fog: {
            density: 0.5,
            heightRate: 0,
            color: [0.01, 0.02, 0.03],
            volumeMedium: [2, 0, 1, 0],
            volumeRange: [60, 1],
          },
        })
      ).output,
      reference({ transmittance: Math.exp(-rayLength) }),
      "source volume replaces near constant fog fill",
    );
    const model = [...identity];
    model[0] = 2;
    model[10] = 0.5;
    const n = Math.SQRT1_2,
      normal = [-n, 0, -n],
      adjustedNZ = 2 / Math.hypot(0.5, 2);
    const scaledReference = (() => {
      const length = Math.hypot(1 / 32, 1 / 32, 1.25);
      const dot = (0.5 / Math.hypot(0.5, 2) / 32 + adjustedNZ * 1.25) / length;
      const alpha = 0.35 * (0.04 + 0.96 * (1 - dot));
      return background.map((v, i) =>
        i < 3 ? v * (1 - alpha) + (64 / 255) ** 0.3 * 0.01 * alpha : 1,
      );
    })();
    close(
      (await render({ model, normal })).output,
      scaledReference,
      "inverse-transpose normal under nonuniform scale",
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
