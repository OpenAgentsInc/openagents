import assert from "node:assert/strict";
import test from "node:test";
import { IblTextures, type IblManifest } from "../packages/renderer/ibl.ts";
Object.assign(globalThis, {
  GPUTextureUsage: { COPY_DST: 1, TEXTURE_BINDING: 2 },
});
function fixture(companion = false) {
  const files = new Map<string, Uint8Array>();
  const mips = (tag: string, value: number) =>
    [0, 1].map((level) => {
      const width = 8 >> level,
        height = 4 >> level,
        values = new Uint16Array(width * height * 4).fill(value);
      const url = `/private-assets/${tag}-${level}.bin`;
      files.set(url, new Uint8Array(values.buffer));
      return {
        level,
        width,
        height,
        roughness: level,
        url,
        format: "rgba16float" as const,
      };
    });
  const manifest: IblManifest = {
    schema: "aethyrforge.private-environment-lighting.v1",
    profile: "sh9-ggx-split-sum-v1",
    brdfLut: {
      url: "/private-assets/brdf.bin",
      width: 1,
      height: 1,
      format: "rgba16float",
    },
    scenarios: {
      day: {
        diffuseSH9: Array(36).fill(1),
        worldToEnvironment: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
        specularMips: mips("primary", 0x3c00),
        tint: [1, 1, 1],
        brightness: 1,
        previewScale: 0.01,
        ...(companion
          ? {
              constantFog: {
                diffuseSH9: Array(36).fill(2),
                specularMips: mips("constant", 0x4000),
                source: {
                  url: "/private-assets/constant-source.bin",
                  sha256: "a".repeat(64),
                  width: 16,
                  height: 8,
                },
              },
            }
          : {}),
      },
    },
  };
  files.set(manifest.brdfLut.url, new Uint8Array(8));
  return { manifest, files };
}
function gpu() {
  const textures: any[] = [],
    uploads: any[] = [];
  return {
    textures,
    uploads,
    device: {
      createTexture(descriptor: any) {
        const t = {
          descriptor,
          destroyed: 0,
          destroy() {
            this.destroyed++;
          },
        };
        textures.push(t);
        return t;
      },
      queue: {
        writeTexture(target: any, data: any, layout: any, size: any) {
          uploads.push({
            target,
            data: new Uint8Array(
              data instanceof ArrayBuffer ? data : data.buffer,
              data.byteOffset ?? 0,
              data.byteLength,
            ).slice(),
            layout,
            size,
          });
        },
      },
    } as unknown as GPUDevice,
  };
}
async function withFiles<T>(
  files: Map<string, Uint8Array>,
  run: () => Promise<T>,
) {
  const previous = globalThis.fetch;
  globalThis.fetch = (async (url: any) => {
    const bytes = files.get(String(url));
    return bytes
      ? new Response(new Uint8Array(bytes).buffer)
      : new Response(null, { status: 404 });
  }) as typeof fetch;
  try {
    return await run();
  } finally {
    globalThis.fetch = previous;
  }
}
test("legacy IBL keeps primary values with a zero companion in a two-layer texture", async () => {
  const f = fixture(),
    g = gpu();
  await withFiles(f.files, async () => {
    const owner = await IblTextures.create(g.device, f.manifest),
      day = owner.scenarios.get("day")!;
    assert.equal(day.hasConstantFog, false);
    assert.deepEqual([...day.constantSH], Array(36).fill(0));
    assert.deepEqual([...day.sh], Array(36).fill(1));
    assert.deepEqual((day.texture as any).descriptor.size, [8, 4, 2]);
    assert.deepEqual((owner.fallback as any).descriptor.size, [1, 1, 2]);
    const uploads = g.uploads.filter((u) => u.target.texture === day.texture);
    assert.equal(uploads.length, 2);
    assert.ok(uploads.every((u) => u.target.origin[2] === 0));
    assert.equal(owner.bytes, 24 + 8 + (8 * 4 + 4 * 2) * 8 * 2);
    owner.dispose();
    owner.dispose();
    assert.ok(g.textures.every((t) => t.destroyed === 1));
  });
});
test("constant IBL uploads matching mips to layer one and exposes independent coefficients", async () => {
  const f = fixture(true),
    g = gpu();
  await withFiles(f.files, async () => {
    const owner = await IblTextures.create(g.device, f.manifest),
      day = owner.scenarios.get("day")!;
    assert.equal(day.hasConstantFog, true);
    assert.deepEqual([...day.constantSH], Array(36).fill(2));
    const uploads = g.uploads.filter((u) => u.target.texture === day.texture);
    assert.deepEqual(
      uploads.map((u) => [u.target.mipLevel, u.target.origin[2]]),
      [
        [0, 0],
        [1, 0],
        [0, 1],
        [1, 1],
      ],
    );
    assert.equal(
      new DataView(uploads[2].data.buffer).getUint16(0, true),
      0x4000,
    );
    owner.dispose();
    assert.ok(g.textures.every((t) => t.destroyed === 1));
  });
});
test("mismatched companion layouts, invalid SH and bad payloads clean up partial GPU loads", async () => {
  for (const kind of ["layout", "sh", "missing", "nonfinite"]) {
    const f = fixture(true),
      g = gpu();
    const c = f.manifest.scenarios.day!.constantFog!;
    if (kind === "layout") c.specularMips[1].roughness = 0.9;
    if (kind === "sh") c.diffuseSH9[0] = NaN;
    if (kind === "missing") f.files.delete(c.specularMips[1].url);
    if (kind === "nonfinite")
      new DataView(f.files.get(c.specularMips[0].url)!.buffer).setUint16(
        0,
        0x7c00,
        true,
      );
    await withFiles(f.files, async () => {
      await assert.rejects(IblTextures.create(g.device, f.manifest));
      assert.ok(
        g.textures.every((t) => t.destroyed === 1),
        kind,
      );
    });
  }
});
