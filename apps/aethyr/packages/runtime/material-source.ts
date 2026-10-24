export const MATERIAL_PNG_BYTES = 16 * 1024;
export const MATERIAL_IMAGE_SIZE = 256;
export type MaterialResource = {
  sha256: string;
  bytes: number;
  media_type: "image/png";
};
export type MaterialImage = { resource: MaterialResource; png_base64: string };
export type MaterialLayer = {
  image: MaterialResource | null;
  visible: boolean;
  strength: number;
};
export type MaterialTarget = {
  entity: string;
  asset: string;
  surface: string;
  material: string;
  uv_set: number;
  size: [number, number];
  uv_bounds: [number, number, number, number];
};
export type MaterialDocument = {
  schema: string;
  target: MaterialTarget;
  generated: MaterialLayer;
  handmade: MaterialLayer;
  recipe: unknown;
};
export type MaterialBundle = {
  document: MaterialDocument;
  generated: MaterialImage | null;
  handmade: MaterialImage | null;
};

const utf8 = new TextEncoder();
export function checkMaterialTarget(target: MaterialTarget, snapshot: any) {
  if (
    !target ||
    !/^[A-Za-z0-9:._\/-]{1,128}$/.test(target.entity) ||
    target.asset !== "custom:haunt:werewolf" ||
    target.surface !== "werewolf.face" ||
    target.material !== "custom:haunt:werewolf:face" ||
    target.uv_set !== 0 ||
    JSON.stringify(target.size) !== "[256,256]" ||
    JSON.stringify(target.uv_bounds) !== "[0,0,1,1]" ||
    !(snapshot.entities ?? []).some(
      (e: any) => e.id === target.entity && e.kind === target.asset,
    )
  )
    throw new Error("Select a werewolf face in this world.");
}
export function checkMaterialDocument(
  document: MaterialDocument,
  snapshot: any,
) {
  checkMaterialTarget(document?.target, snapshot);
  if (
    document.schema !== "openagents.verse.material.v1" ||
    utf8.encode(JSON.stringify(document)).length > 4096
  )
    throw new Error("Keep the face material within its supported size.");
  for (const layer of [document.generated, document.handmade]) {
    const image = layer?.image;
    if (
      !layer ||
      typeof layer.visible !== "boolean" ||
      !Number.isFinite(layer.strength) ||
      layer.strength < 0 ||
      layer.strength > 1 ||
      (image &&
        (image.media_type !== "image/png" ||
          !/^[a-f0-9]{64}$/.test(image.sha256) ||
          !Number.isSafeInteger(image.bytes) ||
          image.bytes < 1 ||
          image.bytes > MATERIAL_PNG_BYTES))
    )
      throw new Error("Use a 256 × 256 PNG up to 16 KiB for each face layer.");
  }
}

export function savedMaterialDocuments(snapshot: any): MaterialDocument[] {
  return Object.values(snapshot.authoring?.materialSources ?? {}).map(
    (source: any) => JSON.parse(source.text),
  );
}
