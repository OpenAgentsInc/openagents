import type { CompiledScene } from "../../packages/assets/src/scene.ts";
import { preparePublishedWorld } from "../../packages/runtime/published-world.ts";
import {
  playerProfileFeatures,
  type PlayerArtifact,
} from "../../packages/runtime/player-artifact.ts";
import {
  streamPackageIdentity,
  type StreamPackageManifest,
} from "../../packages/runtime/cell-stream.ts";
import { roomRendererFeatures } from "../../packages/renderer/room-features.ts";
import { selectedRendererFeatures } from "../../packages/renderer/features.ts";
import { viewAngles } from "../../packages/renderer/math.ts";
import type {
  MaterialBundle,
  MaterialDocument,
} from "../../packages/runtime/material-source.ts";

const width = 480,
  height = 300,
  kind = "custom:haunt:werewolf",
  materialId = kind + ":face";
function assert(value: unknown, message: string): asserts value {
  if (!value) throw Error(message);
}
const encode = (value: unknown) =>
  new TextEncoder().encode(JSON.stringify(value));
const sha256 = async (bytes: ArrayBuffer | Uint8Array<ArrayBuffer>) =>
  [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");
function base64(bytes: Uint8Array) {
  let value = "";
  for (let start = 0; start < bytes.length; start += 8192)
    value += String.fromCharCode(...bytes.subarray(start, start + 8192));
  return btoa(value);
}
async function publishedFixture() {
  const source = (await (
    await fetch("/starter/scene.json")
  ).json()) as CompiledScene;
  const day = structuredClone(source),
    night = structuredClone(source);
  for (const [index, scene] of [day, night].entries()) {
    scene.scenario = index ? "night" : "day";
    delete scene.characterURL;
    scene.materials = [
      {
        ...source.materials[0],
        id: materialId,
        name: "Owned face fixture",
        baseColorFactor: [1, 1, 1, 1],
      },
    ];
    scene.meshes = [{ ...structuredClone(source.meshes[0]), id: kind }];
    const floor = structuredClone(
      source.instances.find((item) => item.actor === "courtyard-floor")!,
    );
    assert(floor, "The owned Starter floor was not found.");
    floor.materialOverrides = [0];
    scene.instances = [floor];
    scene.batches = [
      { mesh: 0, firstInstance: 0, instanceCount: 1, materialOverrides: [0] },
    ];
    scene.catalog = "/starter/material-proof-catalog.json";
  }
  const world = {
    schema: "aethyrforge.player-world.v1",
    groundY: 0,
    mode: "fly",
    spawn: { position: [0, 1.6, 6.5], yaw: 0, pitch: 0, mode: "fly" },
    entities: [
      {
        id: "left-face",
        kind,
        position: [-1.1, 0.8, 0],
        yaw: 0,
        scale: 1.4,
        collidable: false,
      },
      {
        id: "right-face",
        kind,
        position: [1.1, 0.8, 0],
        yaw: 0,
        scale: 1.4,
        collidable: false,
      },
    ],
    colliders: [],
    rooms: [],
  };
  const worldURL = "/starter/material-proof-world.json";
  const resources = new Map<string, Uint8Array<ArrayBuffer>>([
    [worldURL, encode(world)],
    ["/starter/material-proof-day.json", encode(day)],
    ["/starter/material-proof-night.json", encode(night)],
    [
      day.catalog!,
      encode({
        schema: "aethyrforge.private-catalog.v1",
        assets: [
          {
            id: kind,
            mesh: 0,
            dimensions: [1, 1, 1],
            bounds: source.meshes[0].bounds,
          },
        ],
      }),
    ],
  ]);
  for (const url of [day.vertices, day.indices, day.vertexColors])
    resources.set(url, new Uint8Array(await (await fetch(url)).arrayBuffer()));
  const files = await Promise.all(
    [...resources].map(async ([url, bytes]) => ({
      url,
      sha256: await sha256(bytes),
      bytes: bytes.byteLength,
      gzipBytes: bytes.byteLength,
    })),
  );
  const total = files.reduce((sum, file) => sum + file.bytes, 0),
    unique = [
      ...new Map(files.map((file) => [file.sha256, file])).values(),
    ].reduce((sum, file) => sum + file.bytes, 0);
  const manifest: StreamPackageManifest = {
    schema: "aethyrforge.stream-package.v1",
    abi: 1,
    identityScope: "stream-descriptor-v1",
    sha256: "0".repeat(64),
    entryCell: "entry",
    cells: [
      {
        id: "entry",
        quality: "source",
        bounds: { min: [-10, -10], max: [10, 10] },
        scenes: {
          day: "/starter/material-proof-day.json",
          night: "/starter/material-proof-night.json",
        },
        resources: files,
      },
    ],
    files,
    totals: {
      rawBytes: total,
      gzipBytes: total,
      uniqueRawBytes: unique,
      uniqueGzipBytes: unique,
    },
  };
  manifest.sha256 = await streamPackageIdentity(manifest);
  const kernel = new Uint8Array(
    await (await fetch("/kernel.wasm")).arrayBuffer(),
  );
  const features = playerProfileFeatures("room");
  const artifact: PlayerArtifact = {
    schema: "aethyrforge.player-artifact.v1",
    abi: 1,
    profile: "room",
    worldId: "owned-material-proof",
    sourceIdentity: await sha256(encode(world)),
    runtime: { abi: 1, url: "/fixture.js", sha256: "1".repeat(64), bytes: 1 },
    kernel: {
      abi: 1,
      url: "/kernel.wasm",
      sha256: await sha256(kernel),
      bytes: kernel.byteLength,
    },
    features,
    content: { package: manifest, entryCell: "entry", worldURL },
    resources: Object.fromEntries(
      features.map((feature) => [
        feature,
        feature === "render"
          ? files.map((file) => file.url)
          : ["rules", "collision"].includes(feature)
            ? [worldURL]
            : [],
      ]),
    ),
  };
  resources.set("/kernel.wasm", kernel);
  const fetcher: typeof fetch = async (input) => {
    const bytes = resources.get(String(input));
    return bytes
      ? new Response(bytes.slice().buffer)
      : new Response("Missing fixture resource", { status: 404 });
  };
  return { artifact, world, fetcher };
}
async function image(draw: (context: CanvasRenderingContext2D) => void) {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 256;
  const context = canvas.getContext("2d")!;
  draw(context);
  const png_base64 = canvas.toDataURL("image/png").split(",")[1],
    bytes = Uint8Array.from(atob(png_base64), (char) => char.charCodeAt(0));
  assert(
    bytes.byteLength <= 16384,
    "The owned PNG exceeds the material image limit.",
  );
  return {
    png_base64,
    resource: {
      media_type: "image/png" as const,
      bytes: bytes.byteLength,
      sha256: await sha256(bytes),
    },
  };
}
function changes(a: Uint8Array, b: Uint8Array) {
  let count = 0,
    other = 0,
    max = 0;
  for (let offset = 0; offset < a.length; offset += 4) {
    let changed = false;
    for (let channel = 0; channel < 4; channel++) {
      const delta = Math.abs(a[offset + channel] - b[offset + channel]);
      max = Math.max(max, delta);
      changed ||= delta > 0;
    }
    if (changed) {
      count++;
      if ((offset / 4) % width >= width / 2) other++;
    }
  }
  return {
    changedPixels: count,
    rightHalfChangedPixels: other,
    maxChannelError: max,
  };
}
export async function runMaterialEditorGpuProbe() {
  const checks: { name: string; details?: unknown }[] = [],
    captures: Record<string, { sha256: string; rgba: string }> = {};
  const check = (value: unknown, name: string, details?: unknown) => {
    assert(value, `${name}: ${JSON.stringify(details)}`);
    checks.push({ name, ...(details === undefined ? {} : { details }) });
  };
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  document.body.append(canvas);
  const fixture = await publishedFixture();
  const prepared = await preparePublishedWorld(
    fixture.artifact,
    canvas.transferControlToOffscreen(),
    {
      fetcher: fixture.fetcher,
      rendererOptions: {
        features: roomRendererFeatures,
        experiment: { backend: "baseline", profiling: false },
      },
    },
  );
  const renderer = prepared.prepared!.renderer;
  try {
    const host = (renderer as any).features;
    check(
      host.createMaterialEditor ===
        selectedRendererFeatures.createMaterialEditor &&
        host.createMeshInspection ===
          selectedRendererFeatures.createMeshInspection,
      "Published Room keeps both trusted editor hooks",
    );
    check(
      !host.character && !host.sky && !host.createExperiment,
      "Published Room keeps character, sky and experiments disabled",
    );
    renderer.resize(width, height);
    const camera = {
      eye: [0, 1.6, 6.5] as [number, number, number],
      ...viewAngles([0, 1.6, 6.5], [0, 0.8, 0]),
      fov: 0.8,
    };
    const environment = {
      ...prepared.prepared!.environment,
      mode: "day" as const,
      exposure: 1,
    };
    const source = {
      entities: fixture.world.entities,
      rooms: [],
      authoring: { materialSources: {} },
    };
    const meshes = new Map([[kind, 0]]);
    const capture = async (name: string) => {
      renderer.render(camera, environment, 0);
      const bytes = new Uint8Array(
        await renderer.capturePixels(0, { camera, env: environment }),
      );
      captures[name] = { sha256: await sha256(bytes), rgba: base64(bytes) };
      return bytes;
    };
    await renderer.updateAuthored(source, meshes);
    const before = await capture("before");
    const generated = await image((context) => {
      context.fillStyle = "#e51012";
      context.fillRect(0, 0, 256, 256);
    });
    const handmade = await image((context) => {
      context.fillStyle = "#1020ed";
      context.fillRect(85, 85, 85, 85);
    });
    const doc: MaterialDocument = {
      schema: "openagents.verse.material.v1",
      target: {
        entity: "left-face",
        asset: kind,
        surface: "werewolf.face",
        material: materialId,
        uv_set: 0,
        size: [256, 256],
        uv_bounds: [0, 0, 1, 1],
      },
      generated: { image: generated.resource, visible: true, strength: 1 },
      handmade: { image: handmade.resource, visible: true, strength: 1 },
      recipe: null,
    };
    let images: MaterialBundle = { document: doc, generated, handmade };
    renderer.creatorImageProvider = () => images;
    const next = structuredClone(source);
    (next.authoring.materialSources as any).face = {
      text: JSON.stringify(doc),
    };
    const staged = await renderer.prepareAuthored(next, meshes);
    check(staged.ready(), "Face plan is ready before activation");
    const stagedPixels = await capture("staged");
    check(
      changes(before, stagedPixels).changedPixels === 0,
      "Staging preserves the displayed frame",
    );
    staged.activate();
    const applied = await capture("applied"),
      difference = changes(before, applied);
    check(
      difference.changedPixels > 100 && difference.rightHalfChangedPixels === 0,
      "Only the selected owned instance changes",
      difference,
    );
    check(
      (renderer as any).authoredDraws[0].material !==
        (renderer as any).authoredDraws[1].material,
      "Selected and untouched instances use distinct material bindings",
    );
    const candidate = structuredClone(next),
      changedDoc = structuredClone(doc);
    changedDoc.generated.strength = 0.5;
    (candidate.authoring.materialSources as any).face = {
      text: JSON.stringify(changedDoc),
    };
    images = { ...images, document: changedDoc };
    const discarded = await renderer.prepareAuthored(candidate, meshes);
    discarded.discard();
    const afterDiscard = await capture("discarded");
    check(
      changes(applied, afterDiscard).changedPixels === 0,
      "Discard retains the working face exactly",
    );
    const invalid = Uint8Array.from([110, 111, 116, 32, 112, 110, 103]),
      invalidDoc = structuredClone(doc);
    invalidDoc.generated.image = {
      media_type: "image/png",
      bytes: invalid.length,
      sha256: await sha256(invalid),
    };
    images = {
      ...images,
      document: invalidDoc,
      generated: {
        resource: invalidDoc.generated.image!,
        png_base64: base64(invalid),
      },
    };
    const rejected = structuredClone(next);
    (rejected.authoring.materialSources as any).face = {
      text: JSON.stringify(invalidDoc),
    };
    let failed = false;
    try {
      await renderer.prepareAuthored(rejected, meshes);
    } catch {
      failed = true;
    }
    check(failed, "Invalid PNG staging fails before activation");
    const afterFailure = await capture("failed");
    check(
      changes(applied, afterFailure).changedPixels === 0,
      "Failed image staging retains the working face exactly",
    );
    renderer.applyMaterialValues({
      [materialId]: { red: 0.2, green: 0.8, blue: 0.3 },
    });
    const scalar = await capture("scalar");
    check(
      changes(applied, scalar).changedPixels > 100,
      "Scalar parameters update the actual uniform buffers",
    );
    renderer.applyMaterialValues({});
    const scalarRestored = await capture("scalar-restored");
    check(
      changes(applied, scalarRestored).changedPixels === 0,
      "Scalar reset restores retained uniform values exactly",
    );
    await renderer.updateAuthored(source, meshes);
    const restored = await capture("restored");
    check(
      changes(before, restored).changedPixels === 0,
      "Removing the face override restores the original pixels exactly",
    );
    return {
      ok: true,
      width,
      height,
      checks,
      captures: Object.entries(captures).map(([name, capture]) => ({
        name,
        width,
        height,
        base64: capture.rgba,
        sha256: capture.sha256,
      })),
      sourceIdentity: fixture.artifact.sourceIdentity,
      ownedPackage: fixture.artifact.content!.package.sha256,
      adapter: renderer.benchmarkInfo().adapter,
      scope:
        "Owned Starter geometry relabeled in memory; actual published-Room host selection and Renderer material integration. No provider, public authoring UI, or performance qualification is tested.",
    };
  } finally {
    renderer.dispose();
    canvas.remove();
  }
}
