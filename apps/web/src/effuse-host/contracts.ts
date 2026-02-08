import { toolContractsExport } from "../../../autopilot-worker/src/tools";
import { moduleContractsExport, signatureContractsExport } from "../../../autopilot-worker/src/dseCatalog";

const json = (body: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(body), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  });

export const handleContractsRequest = async (request: Request): Promise<Response | null> => {
  const url = new URL(request.url);
  if (!url.pathname.startsWith("/api/contracts/")) return null;
  if (request.method !== "GET") return new Response("Method not allowed", { status: 405 });

  switch (url.pathname) {
    case "/api/contracts/tools":
      return json(toolContractsExport(), { status: 200, headers: { "cache-control": "no-store" } });
    case "/api/contracts/signatures":
      return json(signatureContractsExport(), { status: 200, headers: { "cache-control": "no-store" } });
    case "/api/contracts/modules":
      return json(moduleContractsExport(), { status: 200, headers: { "cache-control": "no-store" } });
    default:
      return new Response("Not found", { status: 404, headers: { "cache-control": "no-store" } });
  }
};

