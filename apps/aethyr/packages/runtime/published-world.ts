import type { CompiledCatalog, CompiledScene } from "../assets/src/scene.ts";
import { roomRendererFeatures } from "../renderer/room-features.ts";
import { selectedRendererFeatures } from "../renderer/features.ts";
import { CellStream, type CellStreamSnapshot } from "./cell-stream.ts";
import {
  verifyPlayerArtifact,
  type PlayerArtifact,
  type PlayerArtifactCompatibility,
} from "./player-artifact.ts";
import {
  validatePlayerWorld,
  PLAYER_WORLD_LIMITS,
  type PlayerWorld,
} from "./player-world.ts";
import { prepareWorld, type WorldLoaderOptions } from "./world-loader.ts";

export interface PublishedWorldOptions {
  fetcher?: typeof fetch;
  report?: (message: string) => void;
  compatibility?: PlayerArtifactCompatibility;
  signal?: AbortSignal;
  rendererOptions?: WorldLoaderOptions["rendererOptions"];
}
export interface PublishedWorldStage {
  artifact: PlayerArtifact;
  world?: PlayerWorld;
  catalog?: CompiledCatalog;
  prepared?: Awaited<ReturnType<typeof prepareWorld>>;
  kernelBytes?: ArrayBuffer;
  /** The temporary verification cache is already disposed; high-water counters remain. */
  streamMetrics?: CellStreamSnapshot;
  stageTimings: {
    artifactMs: number;
    kernelMs: number;
    contentMs: number;
    prepareMs: number;
    totalMs: number;
  };
}
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const integer = (value: unknown, max: number): value is number =>
  typeof value === "number" &&
  Number.isSafeInteger(value) &&
  value >= 0 &&
  value <= max;
const hash = async (bytes: Uint8Array<ArrayBuffer> | ArrayBuffer) =>
  [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((n) => n.toString(16).padStart(2, "0"))
    .join("");
function vector(value: unknown, size: number): value is number[] {
  return (
    Array.isArray(value) &&
    value.length === size &&
    value.every(
      (n) =>
        typeof n === "number" &&
        Number.isFinite(n) &&
        Math.abs(n) <= 100_000_000,
    )
  );
}
function aborted(signal?: AbortSignal) {
  if (signal?.aborted)
    throw new DOMException("Opening this world was canceled.", "AbortError");
}
function resource(
  resources: ReadonlyMap<string, Uint8Array<ArrayBuffer>>,
  url: unknown,
): Uint8Array<ArrayBuffer> {
  if (typeof url !== "string" || !resources.has(url))
    throw new Error("The published world is missing a verified resource.");
  return resources.get(url)!;
}
function document(
  resources: ReadonlyMap<string, Uint8Array<ArrayBuffer>>,
  url: unknown,
  limit: number,
): unknown {
  const bytes = resource(resources, url);
  if (bytes.byteLength > limit)
    throw new Error("The published document exceeds its size limit.");
  return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
}
function sceneDocument(
  value: unknown,
  mode: "day" | "night",
  resources: ReadonlyMap<string, Uint8Array<ArrayBuffer>>,
): CompiledScene {
  if (
    !record(value) ||
    value.schema !== "aethyrforge.private-scene.v1" ||
    value.scenario !== mode ||
    value.vertexStride !== 48 ||
    !integer(value.vertexCount, 50_000_000) ||
    !integer(value.indexCount, 150_000_000) ||
    !Array.isArray(value.materials) ||
    value.materials.length > 65_536 ||
    !value.materials.every(record) ||
    !Array.isArray(value.meshes) ||
    value.meshes.length > 65_536 ||
    !Array.isArray(value.instances) ||
    value.instances.length > 2_000_000 ||
    !Array.isArray(value.batches) ||
    value.batches.length > 2_000_000
  )
    throw new Error(
      "Open complete prepared scene geometry for both lighting scenarios.",
    );
  if (
    resource(resources, value.vertices).byteLength !== value.vertexCount * 48 ||
    resource(resources, value.indices).byteLength !== value.indexCount * 4 ||
    resource(resources, value.vertexColors).byteLength !==
      value.vertexCount * 4 ||
    (value.vertexUV1 !== undefined &&
      resource(resources, value.vertexUV1).byteLength !== value.vertexCount * 8)
  )
    throw new Error(
      "The prepared geometry sizes do not match their scene counts.",
    );
  for (const mesh of value.meshes) {
    if (
      !record(mesh) ||
      !Array.isArray(mesh.primitives) ||
      mesh.primitives.length > 65_536 ||
      !record(mesh.bounds) ||
      !vector(mesh.bounds.min, 3) ||
      !vector(mesh.bounds.max, 3) ||
      mesh.bounds.min.some(
        (n, axis) => n > (mesh.bounds as { max: number[] }).max[axis],
      )
    )
      throw new Error("The prepared scene has incomplete mesh bounds.");
    for (const primitive of mesh.primitives)
      if (
        !record(primitive) ||
        !integer(primitive.firstIndex, value.indexCount) ||
        !integer(primitive.indexCount, value.indexCount) ||
        primitive.firstIndex + primitive.indexCount > value.indexCount ||
        primitive.indexCount % 3 !== 0 ||
        !integer(primitive.baseVertex, value.vertexCount) ||
        !integer(primitive.material, value.materials.length - 1) ||
        !integer(primitive.materialSlot, 65_535)
      )
        throw new Error(
          "The prepared scene has an invalid mesh or material index.",
        );
  }
  const overrides = (value: unknown) =>
    Array.isArray(value) &&
    value.length <= 65_536 &&
    value.every(
      (index) => index === -1 || integer(index, valueMaterialCount - 1),
    );
  const valueMaterialCount = value.materials.length;
  for (const instance of value.instances)
    if (
      !record(instance) ||
      !integer(instance.mesh, value.meshes.length - 1) ||
      !vector(instance.model, 16) ||
      !overrides(instance.materialOverrides)
    )
      throw new Error(
        "The prepared scene has an invalid object transform or mesh index.",
      );
  for (const batch of value.batches)
    if (
      !record(batch) ||
      !integer(batch.mesh, value.meshes.length - 1) ||
      !integer(batch.firstInstance, value.instances.length) ||
      !integer(batch.instanceCount, value.instances.length) ||
      batch.firstInstance + batch.instanceCount > value.instances.length ||
      !overrides(batch.materialOverrides)
    )
      throw new Error("The prepared scene has an invalid draw range.");
  return value as unknown as CompiledScene;
}
function validateFeatures(
  artifact: PlayerArtifact,
  day: CompiledScene,
  night: CompiledScene,
) {
  const hasCharacter = artifact.features.includes("character");
  if (
    (!hasCharacter && (day.characterURL || night.characterURL)) ||
    (hasCharacter && !day.characterURL) ||
    (day.characterURL &&
      night.characterURL &&
      day.characterURL !== night.characterURL)
  )
    throw new Error(
      "The published character data does not match this player's features.",
    );
  if (!artifact.features.includes("sky"))
    for (const source of [day, night])
      if (
        [
          source.environmentURL,
          source.lightingEnvironmentURL,
          source.probesURL,
          source.surfaceSkyURL,
        ].some((url) => url !== undefined)
      )
        throw new Error(
          "Open prepared sky data with a player that includes sky support.",
        );
}
function catalogDocument(
  value: unknown,
  day: CompiledScene,
  night: CompiledScene,
): CompiledCatalog {
  if (
    !record(value) ||
    value.schema !== "aethyrforge.private-catalog.v1" ||
    !Array.isArray(value.assets) ||
    value.assets.length > 16_384
  )
    throw new Error("Open a catalog prepared for this player release.");
  const ids = new Set<string>();
  for (const asset of value.assets) {
    if (
      !record(asset) ||
      typeof asset.id !== "string" ||
      !/^[a-zA-Z0-9_.:-]{1,96}$/.test(asset.id) ||
      ids.has(asset.id) ||
      !integer(
        asset.mesh,
        Math.min(day.meshes.length, night.meshes.length) - 1,
      ) ||
      !vector(asset.dimensions, 3) ||
      asset.dimensions.some((n) => n < 0) ||
      !record(asset.bounds) ||
      !vector(asset.bounds.min, 3) ||
      !vector(asset.bounds.max, 3) ||
      asset.bounds.min.some(
        (n, axis) => n > (asset.bounds as { max: number[] }).max[axis],
      )
    )
      throw new Error(
        "A published asset has missing geometry, invalid bounds, or a duplicate name.",
      );
    ids.add(asset.id);
  }
  return value as unknown as CompiledCatalog;
}
async function readPinned(
  response: Response,
  limit: number,
  signal?: AbortSignal,
): Promise<ArrayBuffer> {
  if (!response.ok || !response.body)
    throw new Error("The pinned Rust kernel could not open.");
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    for (;;) {
      aborted(signal);
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > limit)
        throw new Error("The pinned Rust kernel exceeds its declared size.");
      chunks.push(value);
    }
  } finally {
    await reader.cancel();
  }
  if (size !== limit)
    throw new Error("The pinned Rust kernel does not match its declared size.");
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes.buffer;
}
let verifiedFetchInUse = false;
async function withVerifiedResources<T>(
  resources: ReadonlyMap<string, Uint8Array<ArrayBuffer>>,
  signal: AbortSignal,
  prepare: () => Promise<T>,
): Promise<T> {
  if (verifiedFetchInUse)
    throw new Error(
      "Finish opening the current world before preparing another.",
    );
  verifiedFetchInUse = true;
  const previous = globalThis.fetch;
  const origin = new URL(globalThis.location?.href ?? "http://localhost/");
  globalThis.fetch = async (input, init) => {
    aborted(signal);
    aborted(init?.signal ?? undefined);
    const request = input instanceof Request ? input : undefined;
    aborted(request?.signal);
    const method = init?.method ?? request?.method ?? "GET";
    const url = new URL(
      input instanceof Request ? input.url : String(input),
      origin,
    );
    if (
      method.toUpperCase() !== "GET" ||
      url.origin !== origin.origin ||
      url.username ||
      url.password ||
      url.search ||
      url.hash
    )
      throw new Error("This player reads only its verified local asset files.");
    const bytes = resource(resources, url.pathname);
    return new Response(bytes.slice().buffer, {
      headers: {
        "Content-Type": url.pathname.endsWith(".json")
          ? "application/json"
          : "application/octet-stream",
        "Content-Length": String(bytes.byteLength),
      },
    });
  };
  try {
    return await prepare();
  } finally {
    globalThis.fetch = previous;
    verifiedFetchInUse = false;
  }
}

/** Verify and prepare a published world without activating it or retaining its byte cache. */
export async function preparePublishedWorld(
  value: unknown,
  canvas: OffscreenCanvas,
  options: PublishedWorldOptions = {},
): Promise<PublishedWorldStage> {
  const started = performance.now();
  const artifact = await verifyPlayerArtifact(value, options.compatibility);
  const stageTimings = {
    artifactMs: performance.now() - started,
    kernelMs: 0,
    contentMs: 0,
    prepareMs: 0,
    totalMs: 0,
  };
  aborted(options.signal);
  if (artifact.profile === "empty") {
    stageTimings.totalMs = performance.now() - started;
    return { artifact, stageTimings };
  }
  const fetcher = options.fetcher ?? globalThis.fetch.bind(globalThis);
  const kernelStarted = performance.now();
  const pin = artifact.kernel!;
  const kernelBytes = await readPinned(
    await fetcher(pin.url, { signal: options.signal }),
    pin.bytes,
    options.signal,
  );
  const kernelHash = await hash(kernelBytes);
  if (kernelHash !== pin.sha256)
    throw new Error("The Rust kernel does not match the published player.");
  stageTimings.kernelMs = performance.now() - kernelStarted;
  const contentStarted = performance.now();
  const stream = new CellStream({ fetch: fetcher });
  stream.configure(artifact.content!.package);
  let candidate: PublishedWorldStage["prepared"],
    world: PlayerWorld | undefined,
    catalog: CompiledCatalog | undefined;
  let transferred = false;
  try {
    const result = await stream.load(artifact.content!.entryCell, {
      signal: options.signal,
      prepare: async ({ resources, cell, signal }) => {
        aborted(signal);
        const decodeStarted = performance.now();
        if (
          (await hash(resource(resources, artifact.content!.worldURL))) !==
          artifact.sourceIdentity
        )
          throw new Error(
            "The authored world settings do not match the published source.",
          );
        world = validatePlayerWorld(
          document(
            resources,
            artifact.content!.worldURL,
            PLAYER_WORLD_LIMITS.bytes,
          ),
        );
        const day = sceneDocument(
          document(resources, cell.scenes.day, 64 * 1024 * 1024),
          "day",
          resources,
        );
        const night = sceneDocument(
          document(resources, cell.scenes.night, 64 * 1024 * 1024),
          "night",
          resources,
        );
        validateFeatures(artifact, day, night);
        if (world.presentationURL) resource(resources, world.presentationURL);
        if (day.catalog !== night.catalog)
          throw new Error(
            "Prepare both lighting scenarios with the same published catalog.",
          );
        if (day.catalog)
          catalog = catalogDocument(
            document(resources, day.catalog, 16 * 1024 * 1024),
            day,
            night,
          );
        const kinds = new Set(catalog?.assets.map((asset) => asset.id));
        if (world.entities.some((entity) => !kinds.has(entity.kind)))
          throw new Error(
            "Every published object needs geometry in its prepared catalog.",
          );
        const decodeMs = performance.now() - decodeStarted;
        const prepareStarted = performance.now();
        let rendererOptions = options.rendererOptions;
        if (artifact.profile === "room") {
          const createMeshInspection =
            rendererOptions?.features?.createMeshInspection ??
            selectedRendererFeatures.createMeshInspection;
          const createMaterialEditor =
            rendererOptions?.features?.createMaterialEditor ??
            selectedRendererFeatures.createMaterialEditor;
          rendererOptions = {
            ...rendererOptions,
            features:
              createMeshInspection || createMaterialEditor
                ? {
                    ...roomRendererFeatures,
                    createMeshInspection,
                    createMaterialEditor,
                  }
                : roomRendererFeatures,
          };
        }
        candidate = await withVerifiedResources(resources, signal, () =>
          prepareWorld(day, night, canvas, {
            report: options.report ?? (() => {}),
            presentationURL: world!.presentationURL,
            rendererOptions,
          }),
        );
        stageTimings.prepareMs = performance.now() - prepareStarted;
        aborted(signal);
        return {
          decodeMs,
          commit() {
            transferred = true;
          },
          dispose() {
            if (!transferred) candidate?.renderer.dispose();
          },
        };
      },
    });
    if (!result.ok) throw new Error(result.message);
    if (!candidate || !world)
      throw new Error("Prepare a complete published world before opening it.");
    stageTimings.contentMs = performance.now() - contentStarted;
    stream.dispose();
    stageTimings.totalMs = performance.now() - started;
    return {
      artifact,
      world,
      catalog,
      prepared: candidate,
      kernelBytes,
      streamMetrics: stream.snapshot(),
      stageTimings,
    };
  } catch (error) {
    candidate?.renderer.dispose();
    throw error;
  } finally {
    stream.dispose();
  }
}
