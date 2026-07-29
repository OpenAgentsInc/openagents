import { describe, expect, test } from "vite-plus/test";

import {
  PRODUCT_DOWNLOAD_RESOLUTION_PATH,
  PRODUCT_DOWNLOAD_RESOLUTION_SCHEMA_ID,
  createProductDownloadResolver,
} from "./product-download-resolver.server";

const desktopResolution = {
  schema: "openagents.desktop.download_resolution.v1",
  source: "release_set_v2",
  channel: "rc",
  version: "0.1.0-rc.25",
  releasedAt: "2026-07-29T22:00:00.000Z",
  releaseNotes: "OpenAgents Desktop release.",
  sourceRevision: "a".repeat(40),
  detection: {
    platform: "darwin",
    architecture: "arm64",
    method: "override",
  },
  availability: "available",
  selected: {
    target: "darwin-arm64",
    format: "dmg",
    version: "0.1.0-rc.25",
    channel: "rc",
    url: "https://updates.openagents.com/desktop/openagents/rc/openagents.dmg",
    sha256: "b".repeat(64),
    byteLength: 42,
    minimumOs: "macOS 13",
    preferred: true,
  },
  alternatives: [],
} as const;

describe("product-scoped signed download resolver", () => {
  test("keeps OpenAgents Desktop on the existing signed resolver", async () => {
    const delegated: string[] = [];
    const resolver = createProductDownloadResolver(async (request) => {
      delegated.push(request.url);
      return Response.json(desktopResolution);
    });
    const response = await resolver.handle(
      new Request(
        `https://openagents.com${PRODUCT_DOWNLOAD_RESOLUTION_PATH}` +
          "?product=openagents-desktop&target=darwin-arm64&format=dmg&channel=rc",
      ),
    );

    expect(response?.status).toBe(200);
    expect(delegated).toEqual([
      "https://openagents.com/api/public/desktop-download" +
        "?target=darwin-arm64&format=dmg&channel=rc",
    ]);
    expect(await response?.json()).toEqual({
      schema: PRODUCT_DOWNLOAD_RESOLUTION_SCHEMA_ID,
      product: "openagents-desktop",
      resolution: desktopResolution,
    });
  });

  test("fails Omega closed without consulting or relabeling the Desktop feed", async () => {
    let delegated = false;
    const resolver = createProductDownloadResolver(async () => {
      delegated = true;
      return Response.json(desktopResolution);
    });
    const response = await resolver.handle(
      new Request(`https://openagents.com${PRODUCT_DOWNLOAD_RESOLUTION_PATH}?product=omega`),
    );
    const body = await response?.json();

    expect(response?.status).toBe(200);
    expect(delegated).toBe(false);
    expect(body).toEqual({
      schema: PRODUCT_DOWNLOAD_RESOLUTION_SCHEMA_ID,
      product: "omega",
      availability: "unavailable",
      reason: "signed_release_not_published",
    });
    expect(body).not.toHaveProperty("url");
    expect(body).not.toHaveProperty("version");
    expect(JSON.stringify(body)).not.toContain("desktop");
  });

  test("rejects product ambiguity and Omega artifact selectors", async () => {
    const resolver = createProductDownloadResolver();
    const responses = await Promise.all(
      [
        "",
        "?product=unknown",
        "?product=omega&target=darwin-arm64",
        "?product=omega&format=dmg",
        "?product=omega&product=openagents-desktop",
        "?product=openagents-desktop&unexpected=true",
      ].map((query) =>
        resolver.handle(
          new Request(`https://openagents.com${PRODUCT_DOWNLOAD_RESOLUTION_PATH}${query}`),
        ),
      ),
    );
    for (const response of responses) {
      expect(response?.status).toBe(400);
      expect(response?.headers.get("cache-control")).toBe("no-store");
    }
  });

  test("does not claim unrelated paths or non-GET requests", async () => {
    const resolver = createProductDownloadResolver();
    await expect(
      resolver.handle(new Request("https://openagents.com/api/public/product-download/other")),
    ).resolves.toBeUndefined();
    const response = await resolver.handle(
      new Request(`https://openagents.com${PRODUCT_DOWNLOAD_RESOLUTION_PATH}?product=omega`, {
        method: "POST",
      }),
    );
    expect(response?.status).toBe(405);
    expect(response?.headers.get("allow")).toBe("GET");
  });
});
