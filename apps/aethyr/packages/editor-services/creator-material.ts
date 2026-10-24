import { MATERIAL_PNG_BYTES, MATERIAL_IMAGE_SIZE } from "./host-budgets.ts";
import type { ProjectDocument } from "./project-document.ts";
import {
  checkMaterialTarget,
  checkMaterialDocument,
  savedMaterialDocuments,
  type MaterialResource,
  type MaterialImage,
  type MaterialLayer,
  type MaterialTarget,
  type MaterialDocument,
  type MaterialBundle,
} from "../runtime/material-source.ts";
export {
  checkMaterialTarget,
  checkMaterialDocument,
  savedMaterialDocuments,
  type MaterialResource,
  type MaterialImage,
  type MaterialLayer,
  type MaterialTarget,
  type MaterialDocument,
  type MaterialBundle,
} from "../runtime/material-source.ts";
export type MaterialSource = ProjectDocument["originals"][number];
const utf8 = new TextEncoder();
export async function materialHash(bytes: Uint8Array) {
  return [
    ...new Uint8Array(
      await crypto.subtle.digest("SHA-256", bytes as Uint8Array<ArrayBuffer>),
    ),
  ]
    .map((n) => n.toString(16).padStart(2, "0"))
    .join("");
}
export async function materialKey(target: MaterialTarget) {
  return materialHash(
    utf8.encode(`${target.entity}\n${target.asset}\n${target.surface}`),
  );
}
export function sameMaterialResource(
  a: MaterialResource | null | undefined,
  b: MaterialResource | null | undefined,
) {
  return a == null || b == null
    ? a == null && b == null
    : a.sha256 === b.sha256 &&
        a.bytes === b.bytes &&
        a.media_type === b.media_type;
}
function sameNumbers(a: unknown, b: unknown) {
  return (
    Array.isArray(a) &&
    Array.isArray(b) &&
    a.length === b.length &&
    a.every((n, i) => n === b[i])
  );
}
export function sameMaterialDocument(a: MaterialDocument, b: MaterialDocument) {
  const x = a.target,
    y = b.target;
  if (
    a.schema !== b.schema ||
    x.entity !== y.entity ||
    x.asset !== y.asset ||
    x.surface !== y.surface ||
    x.material !== y.material ||
    x.uv_set !== y.uv_set ||
    !sameNumbers(x.size, y.size) ||
    !sameNumbers(x.uv_bounds, y.uv_bounds)
  )
    return false;
  for (const name of ["generated", "handmade"] as const) {
    const x = a[name],
      y = b[name];
    if (
      x.visible !== y.visible ||
      x.strength !== y.strength ||
      !sameMaterialResource(x.image, y.image)
    )
      return false;
  }
  if (a.recipe == null || b.recipe == null)
    return a.recipe == null && b.recipe == null;
  const r = a.recipe as Record<string, unknown>,
    s = b.recipe as Record<string, unknown>;
  return (
    [
      "backend",
      "request",
      "seed",
      "area",
      "count",
      "width",
      "angle",
      "healed",
    ].every((k) => r[k] === s[k]) && sameNumbers(r.color, s.color)
  );
}
export function sameProjectMaterialLayers(a: unknown[], b: unknown[]) {
  return (
    a.length === b.length &&
    a.every(
      (value: any) =>
        b.filter(
          (other: any) =>
            value.material === other.material &&
            value.surface === other.surface &&
            value.uv_set === other.uv_set &&
            value.kind === other.kind &&
            sameMaterialResource(value.texture, other.texture),
        ).length === 1,
    )
  );
}
export async function materialSource(
  document: MaterialDocument,
): Promise<MaterialSource> {
  const text = JSON.stringify(document);
  return {
    name: `material-v1-${await materialKey(document.target)}.json`,
    sha256: await materialHash(utf8.encode(text)),
    text,
  };
}
/** Source images stay outside kernel snapshots; bounded references share its undo history. */
export class MaterialImages {
  readonly images = new Map<string, MaterialImage>();
  #bytes = 0;
  async accept(bundle: MaterialBundle, snapshot: any) {
    checkMaterialDocument(bundle.document, snapshot);
    const additions: MaterialImage[] = [];
    for (const name of ["generated", "handmade"] as const) {
      const image = bundle[name],
        reference = bundle.document[name].image;
      if (!sameMaterialResource(image?.resource, reference))
        throw new Error("The face layer does not match its saved image.");
      if (!image) continue;
      if (image.png_base64.length > Math.ceil(MATERIAL_PNG_BYTES / 3) * 4)
        throw new Error(
          "Keep each face PNG at 16 KiB or less. Export the layers before reducing their size.",
        );
      let raw: string;
      try {
        raw = atob(image.png_base64);
      } catch {
        throw new Error("Choose a valid PNG image.");
      }
      const bytes = Uint8Array.from(raw, (c) => c.charCodeAt(0));
      if (
        bytes.length !== reference!.bytes ||
        bytes.length > MATERIAL_PNG_BYTES ||
        (await materialHash(bytes)) !== reference!.sha256
      )
        throw new Error("The face image changed. Choose the original PNG.");
      const bitmap = await createImageBitmap(
        new Blob([bytes], { type: "image/png" }),
      );
      try {
        if (
          bitmap.width !== MATERIAL_IMAGE_SIZE ||
          bitmap.height !== MATERIAL_IMAGE_SIZE
        )
          throw new Error("Use a 256 × 256 PNG for each face layer.");
      } finally {
        bitmap.close();
      }
      if (
        !this.images.has(reference!.sha256) &&
        !additions.some((i) => i.resource.sha256 === reference!.sha256)
      )
        additions.push(structuredClone(image));
    }
    const total = additions.reduce((n, i) => n + i.resource.bytes, 0);
    // Never evict an image needed by undo. Export and reopen to start a fresh session.
    if (this.#bytes + total > 8 * 1024 * 1024)
      throw new Error(
        "This editing session has many image versions. Export your work and reopen it before adding more.",
      );
    for (const image of additions)
      this.images.set(image.resource.sha256, image);
    this.#bytes += total;
  }
  bundle(document: MaterialDocument): MaterialBundle {
    const image = (layer: MaterialLayer) => {
      if (!layer.image) return null;
      const image = this.images.get(layer.image.sha256);
      if (!image)
        throw new Error(
          "A face image is missing. Reopen the saved project with its source images.",
        );
      return structuredClone(image);
    };
    return {
      document: structuredClone(document),
      generated: image(document.generated),
      handmade: image(document.handmade),
    };
  }
}
export function exportMaterialSources(
  document: ProjectDocument,
  snapshot: any,
) {
  document.originals = document.originals.filter(
    (s) => !s.name.startsWith("material-v1-"),
  );
  for (const source of Object.values(
    snapshot.authoring?.materialSources ?? {},
  ) as MaterialSource[]) {
    const material: MaterialDocument = JSON.parse(source.text);
    checkMaterialDocument(material, snapshot);
    document.originals.push(structuredClone(source));
    const key = source.name.slice("material-v1-".length, -".json".length);
    for (const [kind, layer] of [
      ["generated", material.generated],
      ["paint", material.handmade],
    ] as const)
      if (layer.image)
        document.layers.push({
          material: material.target.material,
          surface: `material-${key}`,
          texture: structuredClone(layer.image),
          uv_set: 0,
          kind,
        });
  }
}

export type MaterialViewRequest =
  | { mode: "inspect"; target: MaterialTarget }
  | { mode: "preview"; material: MaterialBundle }
  | { mode: "clear" };
export type MaterialViewResult = { material?: MaterialBundle; frame: number };
