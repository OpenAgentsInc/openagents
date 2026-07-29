import { Exit, Schema } from "effect";

import {
  DESKTOP_DOWNLOAD_RESOLUTION_PATH,
  DesktopDownloadResolutionSchema,
  routeDesktopDownloadRequest,
} from "./desktop-download-resolver.server";

export const PRODUCT_DOWNLOAD_RESOLUTION_PATH = "/api/public/product-download";
export const PRODUCT_DOWNLOAD_RESOLUTION_SCHEMA_ID =
  "openagents.product.download_resolution.v1" as const;

export const productDownloadIds = ["openagents-desktop", "omega"] as const;
export type ProductDownloadId = (typeof productDownloadIds)[number];

const DesktopProductDownloadResolutionSchema = Schema.Struct({
  schema: Schema.Literal(PRODUCT_DOWNLOAD_RESOLUTION_SCHEMA_ID),
  product: Schema.Literal("openagents-desktop"),
  resolution: DesktopDownloadResolutionSchema,
});

const OmegaProductDownloadUnavailableSchema = Schema.Struct({
  schema: Schema.Literal(PRODUCT_DOWNLOAD_RESOLUTION_SCHEMA_ID),
  product: Schema.Literal("omega"),
  availability: Schema.Literal("unavailable"),
  reason: Schema.Literal("signed_release_not_published"),
});

export const ProductDownloadResolutionSchema = Schema.Union([
  DesktopProductDownloadResolutionSchema,
  OmegaProductDownloadUnavailableSchema,
]);
export type ProductDownloadResolution = typeof ProductDownloadResolutionSchema.Type;

const decodeProductDownloadResolution = Schema.decodeUnknownExit(ProductDownloadResolutionSchema);

type DesktopDownloadRoute = (request: Request) => Promise<Response | undefined>;

const noStoreHeaders = { "cache-control": "no-store" } as const;
const desktopQueryKeys = new Set(["product", "channel", "target", "format"]);

const invalidQuery = (): Response =>
  Response.json(
    {
      error: "invalid_query",
      required: ["product"],
      products: productDownloadIds,
    },
    { status: 400, headers: noStoreHeaders },
  );

export const createProductDownloadResolver = (
  routeDesktop: DesktopDownloadRoute = routeDesktopDownloadRequest,
) => {
  const handle = async (request: Request): Promise<Response | undefined> => {
    const url = new URL(request.url);
    if (url.pathname !== PRODUCT_DOWNLOAD_RESOLUTION_PATH) return undefined;
    if (request.method !== "GET") {
      return Response.json(
        { error: "method_not_allowed" },
        { status: 405, headers: { allow: "GET", ...noStoreHeaders } },
      );
    }

    const product = url.searchParams.get("product");
    const hasOneProduct = url.searchParams.getAll("product").length === 1;
    if (product === "omega") {
      if (!hasOneProduct || [...url.searchParams.keys()].some((key) => key !== "product")) {
        return invalidQuery();
      }
      const resolution: ProductDownloadResolution = {
        schema: PRODUCT_DOWNLOAD_RESOLUTION_SCHEMA_ID,
        product,
        availability: "unavailable",
        reason: "signed_release_not_published",
      };
      return Response.json(resolution, { headers: noStoreHeaders });
    }
    if (
      !hasOneProduct ||
      product !== "openagents-desktop" ||
      [...url.searchParams.keys()].some((key) => !desktopQueryKeys.has(key))
    ) {
      return invalidQuery();
    }

    url.pathname = DESKTOP_DOWNLOAD_RESOLUTION_PATH;
    url.searchParams.delete("product");
    const desktopResponse = await routeDesktop(new Request(url, request));
    if (desktopResponse === undefined) {
      return Response.json(
        { error: "desktop_resolver_unavailable" },
        { status: 503, headers: noStoreHeaders },
      );
    }
    if (!desktopResponse.ok) {
      return Response.json(
        { error: "desktop_resolution_rejected", product },
        { status: desktopResponse.status, headers: noStoreHeaders },
      );
    }
    const candidate = {
      schema: PRODUCT_DOWNLOAD_RESOLUTION_SCHEMA_ID,
      product,
      resolution: await desktopResponse.json(),
    };
    const decoded = decodeProductDownloadResolution(candidate);
    if (!Exit.isSuccess(decoded)) {
      return Response.json(
        { error: "desktop_resolution_invalid" },
        { status: 503, headers: noStoreHeaders },
      );
    }
    return Response.json(decoded.value, {
      status: desktopResponse.status,
      headers: noStoreHeaders,
    });
  };

  return { handle };
};

let defaultResolver: ReturnType<typeof createProductDownloadResolver> | undefined;

export const routeProductDownloadRequest = (request: Request): Promise<Response | undefined> => {
  defaultResolver ??= createProductDownloadResolver();
  return defaultResolver.handle(request);
};
