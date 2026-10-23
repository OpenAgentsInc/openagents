import { parseGlb, parseGlbMeshes, preflightGlbMeshes } from "./asset-compiler.ts";
import type { GlbContainer, ImageDescription, SourceRecord } from "./asset-compiler.ts";
import { MAX_OUTPUT_BYTES, MAX_SOURCE_BYTES, MAX_SOURCE_COUNT, assetPath, sourceKey, validateImportedAsset } from "./asset-import-protocol.ts";
import type { OwnedAssetDescription, OwnedMaterialDescription, OwnedMeshDescription, SourceBytes } from "./asset-import-protocol.ts";
import { sourceBytesHash } from "./protocol.ts";

type ObjectValue = Record<string, unknown>;
export type EncodedImageInfo = { mime: "image/png" | "image/jpeg"; width: number; height: number; outputBytes: number };
const fail = (message: string): never => { throw new Error(message); };
const object = (value: unknown): ObjectValue => value && typeof value === "object" && !Array.isArray(value) ? value as ObjectValue : fail("Expected an asset object.");
const array = (value: unknown, limit: number): unknown[] => value === undefined ? [] : Array.isArray(value) && value.length <= limit ? value : fail("The asset contains too many or invalid entries.");
const integer = (value: unknown): number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : fail("An asset offset or count is invalid.");
const at = (values: unknown[], index: unknown): ObjectValue => object(values[integer(index)]);
const viewOf = (bytes: Uint8Array) => new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
function dimensions(mime: EncodedImageInfo["mime"], width: number, height: number): EncodedImageInfo {
  if (!width || !height || width > 4096 || height > 4096 || width * height * 4 > 4 * 1024 * 1024) fail("The image exceeds the 4 MiB decoded image limit.");
  return { mime, width, height, outputBytes: width * height * 4 };
}
const crcTable = Uint32Array.from({ length: 256 }, (_, index) => {
  let value = index; for (let bit = 0; bit < 8; bit++) value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
  return value >>> 0;
});
function crc(bytes: Uint8Array): number {
  let value = 0xffffffff; for (const byte of bytes) value = crcTable[(value ^ byte) & 255] ^ (value >>> 8);
  return (value ^ 0xffffffff) >>> 0;
}

/** Inspect encoded headers and framing before a browser decoder allocates pixels. */
export function inspectEncodedImage(bytes: Uint8Array): EncodedImageInfo {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength < 4 || bytes.byteLength > MAX_SOURCE_BYTES) fail("The encoded image is empty, truncated, or too large.");
  const view = viewOf(bytes);
  if (bytes[0] === 137 && bytes[1] === 80 && bytes[2] === 78 && bytes[3] === 71) {
    if (bytes.byteLength < 33 || ![137, 80, 78, 71, 13, 10, 26, 10].every((value, i) => bytes[i] === value)) fail("The PNG header is truncated or invalid.");
    let cursor = 8, info: EncodedImageInfo | undefined, data = false, dataBytes = 0, dataEnded = false, palette = false, color = -1;
    while (cursor < bytes.byteLength) {
      if (cursor + 12 > bytes.byteLength) fail("The PNG chunk is truncated.");
      const size = view.getUint32(cursor), end = cursor + 12 + size;
      if (end > bytes.byteLength) fail("The PNG chunk is truncated.");
      const name = String.fromCharCode(...bytes.subarray(cursor + 4, cursor + 8));
      if (!/^[A-Za-z]{4}$/.test(name) || crc(bytes.subarray(cursor + 4, end - 4)) !== view.getUint32(end - 4)) fail("The PNG chunk checksum or type is invalid.");
      if (!info && name !== "IHDR") fail("The PNG header must be first.");
      if (name === "IHDR") {
        if (info || size !== 13) fail("The PNG header is invalid.");
        info = dimensions("image/png", view.getUint32(cursor + 8), view.getUint32(cursor + 12));
        const depth = bytes[cursor + 16]; color = bytes[cursor + 17];
        const allowed: Record<number, number[]> = { 0: [1, 2, 4, 8, 16], 2: [8, 16], 3: [1, 2, 4, 8], 4: [8, 16], 6: [8, 16] };
        if (!allowed[color]?.includes(depth) || bytes[cursor + 18] !== 0 || bytes[cursor + 19] !== 0 || bytes[cursor + 20] > 1) fail("The PNG format is unsupported.");
      } else if (name === "PLTE") {
        if (palette || data || [0, 4].includes(color) || size < 3 || size > 768 || size % 3) fail("The PNG palette is invalid.");
        palette = true;
      } else if (name === "IDAT") {
        if (dataEnded || (color === 3 && !palette)) fail("The PNG image data is invalid.");
        data = true; dataBytes += size;
      } else if (name === "IEND") {
        if (size || !data || !dataBytes || end !== bytes.byteLength) fail("The PNG end marker is invalid.");
        return info!;
      } else {
        if (name[0] === name[0].toUpperCase()) fail("The PNG uses an unsupported critical chunk.");
        if (data) dataEnded = true;
      }
      cursor = end;
    }
    fail("The PNG end marker is missing.");
  }
  if (bytes[0] !== 255 || bytes[1] !== 216) fail("Choose a PNG or JPEG image.");
  let cursor = 2, info: EncodedImageInfo | undefined, scanned = false, quantized = false, huffman = false;
  const components = new Set<number>();
  while (cursor < bytes.byteLength) {
    if (bytes[cursor++] !== 255) fail("The JPEG marker is invalid.");
    while (bytes[cursor] === 255) cursor++;
    const marker = bytes[cursor++];
    if (marker === 217) {
      if (!info || !scanned || cursor !== bytes.byteLength) fail("The JPEG image is incomplete.");
      return info;
    }
    if (!marker || marker === 216 || (marker >= 208 && marker <= 215) || cursor + 2 > bytes.byteLength) fail("The JPEG marker is invalid or truncated.");
    const length = view.getUint16(cursor), end = cursor + length;
    if (length < 2 || end > bytes.byteLength) fail("The JPEG segment is truncated.");
    if ([192, 193, 194, 195, 197, 198, 199, 201, 202, 203, 205, 206, 207].includes(marker)) {
      if (info || ![192, 194].includes(marker) || length < 11 || bytes[cursor + 2] !== 8) fail("The JPEG frame format is unsupported.");
      const count = bytes[cursor + 7];
      if (![1, 3].includes(count) || length !== 8 + count * 3) fail("The JPEG component header is invalid.");
      info = dimensions("image/jpeg", view.getUint16(cursor + 5), view.getUint16(cursor + 3));
      for (let i = 0; i < count; i++) { const id = bytes[cursor + 8 + i * 3]; if (components.has(id)) fail("The JPEG components repeat."); components.add(id); }
    }
    if (marker === 219) quantized = true;
    if (marker === 196) huffman = true;
    if (marker === 218) {
      const count = bytes[cursor + 2];
      if (!info || !quantized || !huffman || !count || count > components.size || length !== 6 + count * 2) fail("The JPEG scan header is invalid.");
      const seen = new Set<number>();
      for (let i = 0; i < count; i++) { const id = bytes[cursor + 3 + i * 2]; if (!components.has(id) || seen.has(id)) fail("The JPEG scan component is invalid."); seen.add(id); }
      cursor = end; const began = cursor;
      while (cursor < bytes.byteLength) {
        if (bytes[cursor] !== 255) { cursor++; continue; }
        if (bytes[cursor + 1] === 0 || (bytes[cursor + 1] >= 208 && bytes[cursor + 1] <= 215)) { cursor += 2; continue; }
        break;
      }
      if (cursor === began) fail("The JPEG scan is empty.");
      scanned = true;
    } else cursor = end;
  }
  return fail("The JPEG end marker is missing.");
}

async function checked(input: SourceBytes): Promise<void> {
  sourceKey(input.record);
  if (!(input.bytes instanceof Uint8Array) || input.bytes.byteLength !== input.record.bytes || await sourceBytesHash(input.bytes) !== input.record.hash) fail(`${input.record.path}: source bytes changed.`);
}
function local(root: SourceRecord, uri: unknown, files: readonly SourceBytes[]): SourceBytes {
  if (typeof uri !== "string" || !uri || uri.length > 512) return fail("Choose a project-local asset file.");
  let decoded: string; try { decoded = decodeURIComponent(uri); } catch { return fail("The asset path is invalid."); }
  if (decoded.startsWith("/") || /[\\\0?#:]/.test(decoded)) fail("External and data URLs are unsupported.");
  const path = assetPath(root.path.slice(0, root.path.lastIndexOf("/") + 1) + decoded);
  const matches = files.filter(file => file.record.path === path);
  return matches.length === 1 ? matches[0] : fail(`${path}: choose exactly one saved source file.`);
}
async function decodeImage(bytes: Uint8Array, info: EncodedImageInfo, id: string, source: string): Promise<ImageDescription> {
  if (typeof createImageBitmap !== "function" || typeof OffscreenCanvas === "undefined") fail("Image imports require a browser image decoder.");
  const bitmap = await createImageBitmap(new Blob([new Uint8Array(bytes).buffer], { type: info.mime }),
    { imageOrientation: "none", premultiplyAlpha: "none", colorSpaceConversion: "none" });
  let canvas: OffscreenCanvas | undefined;
  try {
    if (bitmap.width !== info.width || bitmap.height !== info.height) fail("The decoded image size does not match its header.");
    canvas = new OffscreenCanvas(info.width, info.height);
    const context = canvas.getContext("2d"); if (!context) fail("The image could not be read.");
    context.drawImage(bitmap, 0, 0);
    const rgba = context.getImageData(0, 0, info.width, info.height).data;
    return { id, source, mime: info.mime, width: info.width, height: info.height, bytes: new Uint8Array(rgba) };
  } finally { bitmap.close(); if (canvas) canvas.width = canvas.height = 1; }
}

type Attribute = { name: "normals" | "colors" | "uvs"; count: number; width: number; start: number; stride: number };
function attribute(root: ObjectValue, bytes: Uint8Array, index: unknown, name: Attribute["name"], width: number, count: number): Attribute {
  const value = at(array(root.accessors, 256), index), view = at(array(root.bufferViews, 256), value.bufferView);
  const start = integer(view.byteOffset ?? 0) + integer(value.byteOffset ?? 0), stride = integer(view.byteStride ?? width * 4);
  const viewSize = integer(view.byteLength), offset = integer(value.byteOffset ?? 0);
  if (value.componentType !== 5126 || value.type !== `VEC${width}` || value.count !== count || value.normalized === true || value.sparse !== undefined || view.buffer !== 0 ||
    start % 4 || integer(view.byteOffset ?? 0) % 4 || stride % 4 || stride < width * 4 || stride > 252 ||
    integer(view.byteOffset ?? 0) + viewSize > bytes.byteLength || offset + (count - 1) * stride + width * 4 > viewSize) fail(`${name}: unsupported or out-of-bounds FLOAT accessor.`);
  const data = viewOf(bytes);
  for (let row = 0; row < count; row++) for (let axis = 0; axis < width; axis++) {
    const value = data.getFloat32(start + row * stride + axis * 4, true), lower = name === "normals" ? -1 : name === "colors" ? 0 : -1e6, upper = name === "uvs" ? 1e6 : 1;
    if (!Number.isFinite(value) || value < lower || value > upper) fail(`${name}: attribute contains an invalid value.`);
  }
  return { name, width, count, start, stride };
}
function structuralSubset(root: ObjectValue): ObjectValue[] {
  if (object(root.asset).version !== "2.0" || array(root.animations, 64).length || array(root.skins, 64).length) fail("Only static glTF 2 assets are supported.");
  const required = array(root.extensionsRequired, 16);
  if (required.some(name => name !== "KHR_materials_unlit")) fail("The asset requires an unsupported extension.");
  const primitives: ObjectValue[] = [];
  const allMeshes = array(root.meshes, 64);
  for (const raw of allMeshes) {
    const mesh = object(raw); if (mesh.weights !== undefined) fail("Morph targets are unsupported.");
    for (const item of array(mesh.primitives, 64)) { const primitive = object(item); if (primitive.targets !== undefined || primitive.extensions !== undefined) fail("Morph targets and primitive extensions are unsupported."); primitives.push(primitive); }
  }
  if (!primitives.length || primitives.length > 64) fail("Use between 1 and 64 triangle primitives.");
  const nodes = array(root.nodes, 64), parents = new Set<number>();
  for (const raw of nodes) {
    const node = object(raw);
    for (const [name, identity] of [["matrix", [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]], ["translation", [0, 0, 0]], ["rotation", [0, 0, 0, 1]], ["scale", [1, 1, 1]]] as const) {
      if (node[name] !== undefined && JSON.stringify(node[name]) !== JSON.stringify(identity)) fail("Nonidentity node transforms are unsupported.");
    }
    if (node.skin !== undefined || node.weights !== undefined || node.extensions !== undefined) fail("Skins, morph weights, and node extensions are unsupported.");
    for (const child of array(node.children, 64)) {
      const index = integer(child); if (index >= nodes.length || parents.has(index)) fail("Instanced or invalid node hierarchies are unsupported."); parents.add(index);
    }
  }
  if (nodes.length || root.scenes !== undefined) {
    const scenes = array(root.scenes, 64), roots = scenes.length ? array(at(scenes, root.scene ?? 0).nodes, 64).map(integer) : nodes.map((_, index) => index).filter(index => !parents.has(index));
    const visited = new Set<number>(), instances = new Set<number>();
    const walk = (index: number): void => {
      if (index >= nodes.length || visited.has(index)) fail("Instanced or cyclic scene nodes are unsupported."); visited.add(index);
      const node = object(nodes[index]); if (node.mesh !== undefined) { const mesh = integer(node.mesh); if (mesh >= allMeshes.length || instances.has(mesh)) fail("Each imported mesh must appear once."); instances.add(mesh); }
      for (const child of array(node.children, 64)) walk(integer(child));
    };
    for (const root of roots) walk(root);
    if (instances.size !== allMeshes.length) fail("The scene must use every imported mesh exactly once.");
  }
  return primitives;
}

/** Decode bounded local formats while keeping Maya's position/index decoder authoritative. */
export async function compileImportAsset(source: SourceRecord, bytes: Uint8Array, files: readonly SourceBytes[]): Promise<OwnedAssetDescription> {
  try {
    await checked({ record: source, bytes });
    if (!Array.isArray(files) || files.length > MAX_SOURCE_COUNT) fail("The asset uses too many source files.");
    if (source.kind === "image") {
      const info = inspectEncodedImage(bytes);
      return { id: source.id, sources: [source], meshes: [], diagnostics: [], images: [await decodeImage(bytes, info, `${source.id}#image-0`, source.id)] };
    }
    let container: GlbContainer = source.kind === "glb" ? parseGlb(bytes) : { json: JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)) };
    const root = object(container.json), primitives = structuralSubset(root), buffers = array(root.buffers, 1);
    if (buffers.length !== 1) fail("Use exactly one geometry buffer.");
    const buffer = object(buffers[0]), declared = integer(buffer.byteLength), used = new Map([[source.id, source]]);
    let payload = container.bin, bufferSource = source;
    if (source.kind === "gltf") {
      const input = local(source, buffer.uri, files); if (input.record.kind === "image") fail("Choose a saved binary buffer.");
      await checked(input); if (input.bytes.byteLength !== declared) fail(`${input.record.path}: buffer length does not match.`);
      payload = input.bytes; bufferSource = input.record; used.set(input.record.id, input.record);
      container = { json: { ...root, buffers: [{ byteLength: declared }] }, bin: payload };
    } else if (buffer.uri !== undefined) fail("A GLB must use its embedded buffer.");
    if (!payload || declared > payload.byteLength || payload.byteLength - declared > 3) fail("The geometry buffer is truncated or has excess padding.");
    const bounded = payload.subarray(0, declared), plans: Attribute[][] = [], materialIndices = new Set<number>();
    let auxiliaryBytes = 0;
    for (const primitive of primitives) {
      const attributes = object(primitive.attributes), position = at(array(root.accessors, 256), attributes.POSITION), count = integer(position.count);
      if (count < 3 || count > 65_536 || Object.keys(attributes).some(name => !["POSITION", "NORMAL", "COLOR_0", "TEXCOORD_0"].includes(name))) fail("The mesh attributes are unsupported.");
      const entries: Attribute[] = [];
      for (const [key, name, width] of [["NORMAL", "normals", 3], ["COLOR_0", "colors", 3], ["TEXCOORD_0", "uvs", 2]] as const) if (attributes[key] !== undefined) {
        const plan = attribute(root, bounded, attributes[key], name, width, count); entries.push(plan); auxiliaryBytes += count * width * 4;
      }
      plans.push(entries); if (primitive.material !== undefined) materialIndices.add(integer(primitive.material));
    }
    const materials: OwnedMaterialDescription[] = [], imagePlans = new Map<number, { bytes: Uint8Array; info: EncodedImageInfo; source: string }>();
    const allMaterials = array(root.materials, 64), allImages = array(root.images, 16), textures = array(root.textures, 64);
    for (const index of materialIndices) {
      const material = at(allMaterials, index), pbr = material.pbrMetallicRoughness === undefined ? {} : object(material.pbrMetallicRoughness);
      if (material.alphaMode !== undefined && material.alphaMode !== "OPAQUE" || material.normalTexture !== undefined || material.occlusionTexture !== undefined || material.emissiveTexture !== undefined) fail("This material uses unsupported effects.");
      const factor = pbr.baseColorFactor ?? [1, 1, 1, 1];
      if (!Array.isArray(factor) || factor.length !== 4 || factor.some(value => typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) || material.doubleSided !== undefined && typeof material.doubleSided !== "boolean") fail("The material color is invalid.");
      const extensions = material.extensions === undefined ? {} : object(material.extensions);
      if (Object.keys(extensions).some(name => name !== "KHR_materials_unlit")) fail("This material extension is unsupported.");
      let baseColorTexture: string | undefined;
      if (pbr.baseColorTexture !== undefined) {
        const binding = object(pbr.baseColorTexture), texture = at(textures, binding.index), imageIndex = integer(texture.source);
        if (binding.texCoord !== undefined && binding.texCoord !== 0 || binding.extensions !== undefined || texture.sampler !== undefined || texture.extensions !== undefined) fail("Only default base-color texture coordinates are supported.");
        if (!imagePlans.has(imageIndex)) {
          const image = at(allImages, imageIndex); let encoded: Uint8Array, record = bufferSource;
          if (image.uri !== undefined) { const input = local(source, image.uri, files); if (input.record.kind !== "image") fail("Choose a saved PNG or JPEG source."); await checked(input); encoded = input.bytes; record = input.record; used.set(record.id, record); }
          else { const view = at(array(root.bufferViews, 256), image.bufferView), start = integer(view.byteOffset ?? 0), length = integer(view.byteLength); if (view.buffer !== 0 || view.byteStride !== undefined || start + length > bounded.byteLength) fail("The embedded image view is invalid."); encoded = bounded.subarray(start, start + length); }
          let info: EncodedImageInfo; try { info = inspectEncodedImage(encoded); } catch (error) { throw new Error(`${record.path}: ${String(error instanceof Error ? error.message : error)}`); }
          if (image.mimeType !== undefined && image.mimeType !== info.mime) fail("The image MIME type does not match its header.");
          imagePlans.set(imageIndex, { bytes: encoded, info, source: record.id }); auxiliaryBytes += info.outputBytes;
        }
        baseColorTexture = `${source.id}#image-${imageIndex}`;
      }
      materials.push({ id: `${source.id}#material-${index}`, baseColor: factor as [number, number, number, number],
        doubleSided: material.doubleSided === true, unlit: Object.hasOwn(extensions, "KHR_materials_unlit"), ...(baseColorTexture ? { baseColorTexture } : {}) });
    }
    if (auxiliaryBytes >= MAX_OUTPUT_BYTES) fail("The converted asset exceeds the 16 MiB output limit.");
    const budget = MAX_OUTPUT_BYTES - auxiliaryBytes;
    if (preflightGlbMeshes(container, budget).outputBytes > budget) fail("The converted asset exceeds the 16 MiB output limit.");
    const meshes: OwnedMeshDescription[] = parseGlbMeshes(source.id, container, budget);
    const data = viewOf(bounded);
    for (let index = 0; index < meshes.length; index++) {
      for (const plan of plans[index]) { const values = new Float32Array(plan.count * plan.width); for (let row = 0; row < plan.count; row++) for (let axis = 0; axis < plan.width; axis++) values[row * plan.width + axis] = data.getFloat32(plan.start + row * plan.stride + axis * 4, true); meshes[index][plan.name] = values; }
      if (primitives[index].material !== undefined) meshes[index].material = `${source.id}#material-${integer(primitives[index].material)}`;
      if (materials.find(material => material.id === meshes[index].material)?.baseColorTexture && !meshes[index].uvs) fail("A textured mesh needs TEXCOORD_0.");
    }
    const images: ImageDescription[] = [];
    for (const [index, image] of imagePlans) images.push(await decodeImage(image.bytes, image.info, `${source.id}#image-${index}`, image.source));
    const result = { id: source.id, sources: [...used.values()], meshes, images, diagnostics: [], ...(materials.length ? { materials } : {}) };
    validateImportedAsset(result, source, [source, ...files.map(file => file.record)]);
    return result;
  } catch (error) { throw new Error(`${source.path}: ${error instanceof Error ? error.message : String(error)}`); }
}
