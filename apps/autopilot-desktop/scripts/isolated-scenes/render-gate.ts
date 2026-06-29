import type {
  PixelRegion,
  PngImage,
} from "../../src/testing/headless-pixel.js"

export type SceneRenderSignature = Readonly<{
  label: string
  minBrightPixels: number
  minDistinctLumaBuckets: number
  region?: PixelRegion
}>

export type SceneRenderGateResult = Readonly<{
  ok: boolean
  label: string
  brightPixels: number
  distinctLumaBuckets: number
  sampledPixels: number
  minBrightPixels: number
  minDistinctLumaBuckets: number
}>

const fullFrame: PixelRegion = { x0: 0, y0: 0, x1: 1, y1: 1 }

export const defaultSceneRenderSignature: SceneRenderSignature = {
  label: "default nonblank scene",
  minBrightPixels: 1,
  minDistinctLumaBuckets: 2,
  region: fullFrame,
}

export const scoreSceneRenderSignature = (
  image: PngImage,
  signature: SceneRenderSignature,
): SceneRenderGateResult => {
  const region = signature.region ?? fullFrame
  const px0 = Math.max(0, Math.floor(region.x0 * image.width))
  const py0 = Math.max(0, Math.floor(region.y0 * image.height))
  const px1 = Math.min(image.width, Math.ceil(region.x1 * image.width))
  const py1 = Math.min(image.height, Math.ceil(region.y1 * image.height))
  let brightPixels = 0
  let sampledPixels = 0
  const buckets = new Set<number>()

  for (let y = py0; y < py1; y += 1) {
    for (let x = px0; x < px1; x += 1) {
      const offset = (y * image.width + x) * 4
      const luma =
        (image.data[offset] ?? 0) +
        (image.data[offset + 1] ?? 0) +
        (image.data[offset + 2] ?? 0)
      if (luma > 80) brightPixels += 1
      buckets.add(Math.floor(luma / 32))
      sampledPixels += 1
    }
  }

  const distinctLumaBuckets = buckets.size
  return {
    ok:
      brightPixels >= signature.minBrightPixels &&
      distinctLumaBuckets >= signature.minDistinctLumaBuckets,
    label: signature.label,
    brightPixels,
    distinctLumaBuckets,
    sampledPixels,
    minBrightPixels: signature.minBrightPixels,
    minDistinctLumaBuckets: signature.minDistinctLumaBuckets,
  }
}

export const assertSceneRendered = (
  image: PngImage,
  signature: SceneRenderSignature,
): SceneRenderGateResult => {
  const result = scoreSceneRenderSignature(image, signature)
  if (!result.ok) {
    throw new Error(
      `Scene render gate failed for ${result.label}: ` +
        `${result.brightPixels}/${result.minBrightPixels} bright pixels, ` +
        `${result.distinctLumaBuckets}/${result.minDistinctLumaBuckets} luma buckets.`,
    )
  }
  return result
}
