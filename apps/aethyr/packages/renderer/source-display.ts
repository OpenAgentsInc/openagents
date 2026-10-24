import type { FilmicControls } from "./filmic.ts";
import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";
import { prepareFilmic } from "./filmic.ts";
import {
  validateDisplayLut,
  verifyDisplayLut,
  type DisplayLutDescriptor,
} from "./display-lut.ts";

export interface SourceDisplayRecipe {
  schema: "aethyrforge.source-display.v1";
  scenario: "day" | "night";
  controls: FilmicControls;
  lut: DisplayLutDescriptor;
}
export interface SourceDisplayData {
  recipe: SourceDisplayRecipe;
  bytes: Uint8Array;
}
export type SourceDisplayProfiles = Partial<
  Record<"day" | "night", SourceDisplayData>
>;

function privateURL(
  url: unknown,
  extension: "json" | "bin",
): asserts url is string {
  if (
    typeof url !== "string" ||
    !new RegExp(`^/private-assets/[A-Za-z0-9_-]+\\.${extension}$`).test(url)
  )
    throw new Error(
      "Prepare the display profile in the private asset directory.",
    );
}

export function validateSourceDisplay(
  value: unknown,
  scenario: "day" | "night",
  expected: FilmicControls,
): SourceDisplayRecipe {
  const source = value as SourceDisplayRecipe | null;
  if (
    !source ||
    source.schema !== "aethyrforge.source-display.v1" ||
    source.scenario !== scenario ||
    !source.controls
  )
    throw new Error("Prepare the display profile for this town scenario.");
  const controls = prepareFilmic(source.controls).controls;
  for (const key of [
    "slope",
    "toe",
    "shoulder",
    "blackClip",
    "whiteClip",
  ] as const)
    if (controls[key] !== expected[key])
      throw new Error(
        "Rebuild the display profile for the current film settings.",
      );
  if (controls.shadowContrast.some((v, i) => v !== expected.shadowContrast[i]))
    throw new Error(
      "Rebuild the display profile for the current shadow settings.",
    );
  const lut = validateDisplayLut(source.lut);
  privateURL(lut.url, "bin");
  return { schema: source.schema, scenario, controls, lut };
}

async function boundedBytes(
  response: Response,
  limit: number,
): Promise<Uint8Array> {
  if (!response.ok || !response.body)
    throw new Error("The prepared display profile could not open.");
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      size += next.value.byteLength;
      if (size > limit) {
        await reader.cancel();
        throw new Error("The prepared display profile exceeds its size limit.");
      }
      chunks.push(next.value);
    }
  } finally {
    reader.releaseLock();
  }
  const result = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return result;
}

export async function loadSourceDisplay(
  url: string,
  scenario: "day" | "night",
  expected: FilmicControls,
  load: (url: string) => Promise<Response> = fetch,
): Promise<SourceDisplayData> {
  privateURL(url, "json");
  const encoded = await readAsset(
    url,
    (response) => boundedBytes(response, 64 * 1024),
    { fetcher: load },
  );
  const recipe = validateSourceDisplay(
    JSON.parse(new TextDecoder().decode(encoded)),
    scenario,
    expected,
  );
  const verified = await readAsset(
    recipe.lut.url,
    async (response) => {
      const data = await boundedBytes(response, recipe.lut.byteLength);
      try {
        return await verifyDisplayLut(recipe.lut, data);
      } catch (error) {
        throw new AssetIntegrityError(
          error instanceof Error
            ? error.message
            : "The world lighting file is incomplete. Try again.",
        );
      }
    },
    { fetcher: load },
  );
  return { recipe, bytes: verified.bytes };
}
