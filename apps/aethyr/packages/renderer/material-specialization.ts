/** Independent stock material features; several families may apply to one surface. */
export const STOCK_MATERIAL_FEATURES = Object.freeze({
  layered: 1,
  paint: 2,
  terrain: 4,
  vegetation: 8,
  glass: 16,
  flame: 32,
  creator: 64,
  normal: 128,
  mra: 256,
});
export type StockMaterialFeatures = {
  -readonly [K in keyof typeof STOCK_MATERIAL_FEATURES]: boolean;
};
export const STOCK_MATERIAL_DYNAMIC = 0xffffffff;

/** Derive these booleans from the same packed flags used by the material uniform. */
export function stockMaterialConstants(features: StockMaterialFeatures): {
  AF_MATERIAL_FEATURES: number;
} {
  const names = Object.keys(
    STOCK_MATERIAL_FEATURES,
  ) as (keyof StockMaterialFeatures)[];
  if (
    !features ||
    typeof features !== "object" ||
    Array.isArray(features) ||
    Object.keys(features).length !== names.length ||
    Object.keys(features).some(
      (name) => !names.includes(name as keyof StockMaterialFeatures),
    ) ||
    names.some((name) => typeof features[name] !== "boolean")
  )
    throw new Error(
      "Choose an explicit enabled flag for every stock material feature.",
    );
  return {
    AF_MATERIAL_FEATURES: names.reduce(
      (mask, name) =>
        mask | (features[name] ? STOCK_MATERIAL_FEATURES[name] : 0),
      0,
    ),
  };
}

/** The default retains dynamic uniforms for custom shader edits and generic pipelines. */
export const STOCK_MATERIAL_FEATURE_WGSL = `
override AF_RASTER_FLAGS:u32 = 4294967295u;
fn stockRasterFlag(bit:u32,dynamic:bool)->bool{
 if AF_RASTER_FLAGS==4294967295u{return dynamic;}
 return (AF_RASTER_FLAGS&bit)!=0u;
}
override AF_MATERIAL_FEATURES:u32 = ${STOCK_MATERIAL_DYNAMIC}u;
fn stockMaterialFeature(bit:u32,dynamic:bool)->bool{
 if AF_MATERIAL_FEATURES==${STOCK_MATERIAL_DYNAMIC}u{return dynamic;}
 return (AF_MATERIAL_FEATURES&bit)!=0u;
}
`;
