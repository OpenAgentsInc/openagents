// Original GPU fixtures qualify clustered selection, falloff, and shadow identity.
// Run: openagents browser run -- node tools/verify-clustered-lights.mjs
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
    "<!doctype html><title>Owned clustered light qualification</title>",
  );
});
await server.listen();
let browser, context;
try {
  browser = await chromium.connectOverCDP(
    process.env.OPENAGENTS_CHROME_WS ||
      `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
  );
  context = await browser.newContext();
  const page = await context.newPage();
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { WGSL_CLUSTERED_LIGHTING } =
      await import("/packages/renderer/clustered-lighting.ts");
    const { buildClusteredLights, CLUSTER_HEADER } =
      await import("/packages/renderer/clustered-lights.ts");
    const { inverse, lookAt, multiply, normalize, orthographic, perspective } =
      await import("/packages/renderer/math.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("WebGPU is required for this check.");
    const device = await adapter.requestDevice();
    const errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const module = device.createShaderModule({
      code: `
struct Frame{eye:vec4<f32>};
@group(0)@binding(0)var<uniform>frame:Frame;
fn safeNormalize(v:vec3<f32>)->vec3<f32>{return v/max(length(v),0.000001);}
fn brdf(n:vec3<f32>,view:vec3<f32>,l:vec3<f32>,base:vec3<f32>,rough:f32,metal:f32,dielectric:f32)->vec3<f32>{return vec3(max(dot(n,l),0.0));}
fn localVisibility(index:u32,world:vec3<f32>,geometric:vec3<f32>)->f32{return 0.1+f32(index)/10000.0;}
${WGSL_CLUSTERED_LIGHTING}
struct Probe{world:vec4<f32>,normal:vec4<f32>,pixel:vec4<f32>,subsurface:vec4<f32>};
@group(1)@binding(0)var<storage,read>probes:array<Probe>;
@group(1)@binding(1)var<storage,read_write>output:array<vec4<f32>>;
@compute @workgroup_size(64)fn qualify(@builtin(global_invocation_id)id:vec3<u32>){
 if id.x>=arrayLength(&probes){return;}let p=probes[id.x];
 let value=clusteredSurface(p.pixel.xy,p.world.xyz,p.normal.xyz,safeNormalize(frame.eye.xyz-p.world.xyz),vec3(1.0),0.5,0.0,0.04,p.subsurface.xyz,p.normal.xyz);
 output[id.x]=vec4(value,1.0);
}`,
    });
    const compilation = await module.getCompilationInfo();
    const shaderErrors = compilation.messages
      .filter((m) => m.type === "error")
      .map((m) => `${m.lineNum}:${m.linePos} ${m.message}`);
    if (shaderErrors.length) {
      device.destroy();
      return { shaderErrors, errors };
    }
    const pipeline = await device.createComputePipelineAsync({
      layout: "auto",
      compute: { module, entryPoint: "qualify" },
    });
    const width = 257,
      height = 133,
      tileSize = 32,
      near = 0.1,
      far = 128,
      depthSlices = 12;
    const dot = (a, b) => a.reduce((sum, n, i) => sum + n * b[i], 0);
    const sub = (a, b) => a.map((v, i) => v - b[i]);
    const project = (m, p) =>
      [0, 1, 2, 3].map(
        (r) => m[r] * p[0] + m[r + 4] * p[1] + m[r + 8] * p[2] + m[r + 12],
      );
    const unproject = (m, p) => {
      const q = project(m, p);
      return q.slice(0, 3).map((v) => v / q[3]);
    };
    const eye = [17, 9, 23],
      target = [-3, 2, -8],
      forward = normalize(sub(target, eye)),
      view = lookAt(eye, target, [0.2, 1, 0.1]),
      projection = perspective(1.2, width / height, 0.05, 512),
      offAxis = projection.slice(),
      reflected = new Float32Array([
        -1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
      ]);
    offAxis[8] = 0.31;
    offAxis[9] = -0.17;
    const cameras = [
      {
        name: "perspective",
        eye: [0, 0, 0],
        forward: [0, 0, -1],
        matrix: perspective(1.2, width / height, 0.05, 512),
      },
      {
        name: "translated-rolled",
        eye,
        forward,
        matrix: multiply(projection, view),
      },
      {
        name: "mirrored",
        eye,
        forward,
        matrix: multiply(reflected, multiply(projection, view)),
      },
      { name: "off-axis", eye, forward, matrix: multiply(offAxis, view) },
      {
        name: "orthographic",
        eye,
        forward,
        matrix: multiply(orthographic(-42, 42, -22, 22, 0.05, 512), view),
      },
    ];
    function pointAt(camera, inv, x, y, depth) {
      const clip = [(2 * x) / width - 1, 1 - (2 * y) / height],
        a = unproject(inv, [...clip, 0]),
        b = unproject(inv, [...clip, 1]),
        delta = sub(b, a),
        t =
          (depth - dot(sub(a, camera.eye), camera.forward)) /
          dot(delta, camera.forward);
      return a.map((v, i) => Math.fround(v + delta[i] * t));
    }
    function samplesFor(camera) {
      const inv = inverse(camera.matrix),
        depths = [0.055, 0.075, 160, 300];
      for (let i = 0; i <= depthSlices; i++) {
        const d = near * Math.exp((i * Math.log(far / near)) / depthSlices);
        depths.push(d * (1 - 0.00001), d, d * (1 + 0.00001));
      }
      const pixels = [
        [0.001, 0.001],
        [width - 0.001, height - 0.001],
        [width / 2, height / 2],
      ];
      for (let x = tileSize; x < width; x += tileSize)
        for (const epsilon of [-0.001, 0.001])
          pixels.push([x + epsilon, ((x * 3) % (height - 2)) + 1]);
      for (let y = tileSize; y < height; y += tileSize)
        for (const epsilon of [-0.001, 0.001])
          pixels.push([((y * 5) % (width - 2)) + 1, y + epsilon]);
      for (let x = tileSize; x < width; x += tileSize)
        for (let y = tileSize; y < height; y += tileSize)
          pixels.push([x + 0.001, y - 0.001]);
      const samples = [];
      for (const depth of depths)
        for (const [x, y] of pixels) {
          const world = pointAt(camera, inv, x, y, depth),
            q = project(camera.matrix, world);
          if (q[3] <= 0 || q[2] < 0 || q[2] > q[3]) continue;
          const pixel = [
              ((q[0] / q[3] + 1) * width) / 2,
              ((1 - q[1] / q[3]) * height) / 2,
            ].map(Math.fround),
            i = samples.length;
          samples.push({
            world,
            pixel,
            normal: normalize([0.2 + (i % 3), 0.6, i % 2 ? 1 : -1]).map(
              Math.fround,
            ),
            subsurface: i % 7 === 0 ? [0.12, 0.03, 0.07] : [0, 0, 0],
          });
        }
      return samples;
    }
    function fixtures(camera, samples) {
      let seed = 0x812e1;
      const random = () =>
        (seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296;
      const lights = [],
        shadows = new Map();
      for (let i = 0; i < 488; i++) {
        const anchor = samples[(i * 73) % samples.length].world,
          range = i < 12 ? 1000 : 0.3 + 60 * random(),
          position = anchor.map((v) =>
            Math.fround(v + (random() - 0.5) * range),
          ),
          direction = normalize(sub(anchor, position)),
          kind = i % 3 === 0 ? "spot" : "point",
          legacy = i % 4 === 0,
          intensityUnits = legacy
            ? "unitless"
            : ["candelas", "lumens", "unitless"][i % 3];
        lights.push({
          id: `owned-${i}`,
          label: `Owned light ${i}`,
          kind,
          position,
          direction,
          color: [0.1 + random(), 0.1 + random(), 0.1 + random()],
          intensity: legacy
            ? 1 + random() * 4
            : intensityUnits === "unitless"
              ? 2000
              : 2 + random() * 8,
          intensityUnits,
          range,
          enabled: true,
          castShadows: i % 5 !== 0,
          inverseSquaredFalloff: !legacy,
          falloffExponent: i % 11 === 0 ? 0 : 1 + (i % 8),
          innerConeAngleRadians: i % 13 === 0 ? 0.6 : 0.2,
          outerConeAngleRadians: 0.6,
          useTemperature: false,
          raw: {},
        });
        shadows.set(`owned-${i}`, 1 + ((i * 37) % 5000));
      }
      // Interleaved non-local and disabled records must not shift shadow identity.
      lights.splice(17, 0, {
        ...lights[0],
        id: "disabled",
        enabled: false,
        intensity: 1e7,
      });
      lights.splice(51, 0, {
        ...lights[1],
        id: "directional",
        kind: "directional",
        intensity: 1e7,
      });
      // Near-plane and far-end lights exercise conservative, unclipped slice bounds.
      lights[1] = {
        ...lights[1],
        position: camera.eye.map(Math.fround),
        range: 1,
      };
      return { lights, shadows };
    }
    // Independent scalar all-source-light reference; it never uses cluster lists or records.
    function brute(lights, shadows, sample, shadowed = true) {
      const sum = [0, 0, 0];
      for (const light of lights) {
        if (!light.enabled || light.kind === "directional") continue;
        const p = light.position.map(Math.fround),
          range = Math.fround(light.range),
          delta = sub(p, sample.world),
          d2 = dot(delta, delta),
          distance = Math.sqrt(d2);
        if (distance >= range) continue;
        const l = delta.map((v) => v / Math.max(distance, 0.001));
        let strength = light.intensity;
        if (light.inverseSquaredFalloff !== false) {
          if (light.intensityUnits === "unitless") strength *= 0.0016;
          else if (light.intensityUnits === "lumens")
            strength /=
              light.kind === "spot"
                ? 2 * Math.PI * (1 - Math.cos(light.outerConeAngleRadians))
                : 4 * Math.PI;
        }
        strength = Math.fround(strength * 0.01);
        let attenuation =
          light.inverseSquaredFalloff === false
            ? Math.max(1 - d2 / range ** 2, 0) **
              Math.fround(Math.max(light.falloffExponent, 0.0001))
            : Math.max(1 - (distance / range) ** 4, 0) ** 2 / (d2 + 0.0001);
        if (light.kind === "spot") {
          const direction = normalize(light.direction).map(Math.fround),
            angle = -dot(l, direction),
            inner = Math.fround(Math.cos(light.innerConeAngleRadians)),
            outer = Math.fround(Math.cos(light.outerConeAngleRadians));
          attenuation *=
            inner - outer <= 0.00001
              ? Number(angle >= outer)
              : Math.max(0, Math.min(1, (angle - outer) / (inner - outer))) **
                2;
        }
        const nl = Math.max(dot(sample.normal, l), 0),
          back = Math.max(-dot(sample.normal, l), 0),
          shadow =
            shadowed && light.castShadows ? shadows.get(light.id) : undefined,
          visibility = shadow === undefined ? 1 : 0.1 + shadow / 10000;
        for (let c = 0; c < 3; c++)
          sum[c] +=
            (nl + sample.subsurface[c] * back * 0.35) *
            Math.fround(light.color[c]) *
            strength *
            attenuation *
            visibility;
      }
      return sum;
    }
    const frame = device.createBuffer({
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const reports = [];
    let totalComparisons = 0,
      worstRelative = 0,
      worstAbsolute = 0;
    for (const [cameraIndex, camera] of cameras.entries()) {
      const samples = samplesFor(camera),
        { lights, shadows } = fixtures(camera, samples),
        sourceSets =
          cameraIndex === 0
            ? [
                { name: "empty", lights: [] },
                {
                  name: "disabled-and-directional",
                  lights: lights.filter(
                    (l) => !l.enabled || l.kind === "directional",
                  ),
                },
                { name: "mixed-488", lights },
              ]
            : [{ name: "mixed-488", lights }];
      const packed = new Float32Array(samples.length * 16);
      samples.forEach((s, i) => {
        packed.set(
          [...s.world, 0, ...s.normal, 0, ...s.pixel, 0, 0, ...s.subsurface, 0],
          i * 16,
        );
      });
      const probes = device.createBuffer({
          size: packed.byteLength,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        }),
        output = device.createBuffer({
          size: samples.length * 16,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        }),
        readback = device.createBuffer({
          size: output.size,
          usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });
      device.queue.writeBuffer(frame, 0, new Float32Array([...camera.eye, 0]));
      device.queue.writeBuffer(probes, 0, packed);
      const group1 = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(1),
        entries: [
          { binding: 0, resource: { buffer: probes } },
          { binding: 1, resource: { buffer: output } },
        ],
      });
      for (const set of sourceSets) {
        const expected = samples.map((s) => brute(set.lights, shadows, s));
        let referencePixels;
        for (const maxIndices of [4194304, 1500, 0]) {
          const data = buildClusteredLights({
            viewProjection: camera.matrix,
            eye: camera.eye,
            forward: camera.forward,
            width,
            height,
            tileSize,
            depthSlices,
            near,
            far,
            lights: set.lights,
            shadowIndices: shadows,
            maxIndices,
          });
          if (data.storage[CLUSTER_HEADER.lightsOffset] !== 20)
            throw new Error(
              "The qualifier expects the 20-word clustered header.",
            );
          if (set.name === "mixed-488" && data.lightIds.length !== 488)
            throw new Error(
              "The fixture must retain all 488 enabled local lights.",
            );
          if (
            set.name === "mixed-488" &&
            maxIndices === 4194304 &&
            data.stats.overflowCellCount !== 0
          )
            throw new Error("The full-list fixture unexpectedly overflowed.");
          if (
            set.name === "mixed-488" &&
            maxIndices === 0 &&
            data.stats.overflowCellCount === 0
          )
            throw new Error(
              "The zero-capacity fixture must exercise overflow.",
            );
          const storage = device.createBuffer({
            size: data.storage.byteLength,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
          });
          device.queue.writeBuffer(storage, 0, data.storage);
          const group0 = device.createBindGroup({
            layout: pipeline.getBindGroupLayout(0),
            entries: [
              { binding: 0, resource: { buffer: frame } },
              { binding: 13, resource: { buffer: storage } },
            ],
          });
          const encoder = device.createCommandEncoder(),
            pass = encoder.beginComputePass();
          pass.setPipeline(pipeline);
          pass.setBindGroup(0, group0);
          pass.setBindGroup(1, group1);
          pass.dispatchWorkgroups(Math.ceil(samples.length / 64));
          pass.end();
          encoder.copyBufferToBuffer(output, 0, readback, 0, output.size);
          device.queue.submit([encoder.finish()]);
          await readback.mapAsync(GPUMapMode.READ);
          const pixels = new Float32Array(readback.getMappedRange().slice(0));
          readback.unmap();
          storage.destroy();
          for (let i = 0; i < samples.length; i++)
            for (let c = 0; c < 3; c++) {
              const actual = pixels[i * 4 + c],
                want = expected[i][c],
                absolute = Math.abs(actual - want),
                relative = absolute / Math.max(Math.abs(want), 0.00001);
              totalComparisons++;
              worstAbsolute = Math.max(worstAbsolute, absolute);
              if (want > 0.0001)
                worstRelative = Math.max(worstRelative, relative);
              if (
                !Number.isFinite(actual) ||
                absolute > 0.00002 + Math.abs(want) * 0.0006
              )
                throw new Error(
                  JSON.stringify({
                    camera: camera.name,
                    fixture: set.name,
                    maxIndices,
                    sample: i,
                    channel: c,
                    actual,
                    expected: want,
                    point: samples[i],
                    stats: data.stats,
                  }),
                );
              if (
                referencePixels &&
                Math.abs(actual - referencePixels[i * 4 + c]) >
                  0.000002 + Math.abs(actual) * 0.00002
              )
                throw new Error(
                  `Overflow changed ${camera.name} sample ${i}, channel ${c}.`,
                );
            }
          referencePixels ??= pixels;
          reports.push({
            camera: camera.name,
            fixture: set.name,
            maxIndices,
            samples: samples.length,
            lights: data.lightIds.length,
            ...data.stats,
          });
        }
        if (set.name === "mixed-488") {
          const delta = samples.reduce(
            (sum, s, i) =>
              sum +
              brute(set.lights, shadows, s, false).reduce(
                (n, v, c) => n + Math.abs(v - expected[i][c]),
                0,
              ),
            0,
          );
          if (delta < 1)
            throw new Error(
              "Shadow-index sentinels did not affect the fixture.",
            );
        }
      }
      probes.destroy();
      output.destroy();
      readback.destroy();
    }
    await device.queue.onSubmittedWorkDone();
    frame.destroy();
    device.destroy();
    return {
      shaderErrors,
      errors,
      reports,
      totalComparisons,
      worstAbsolute,
      worstRelative,
    };
  });
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  assert.equal(result.reports.length, 21);
  assert.ok(result.totalComparisons > 100000);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context?.close();
  await browser?.close();
  await server.close();
}
