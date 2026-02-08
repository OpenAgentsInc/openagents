export const OA_REQUEST_ID_HEADER = "x-oa-request-id";

// Keep this stable + short so it is easy to paste into `wrangler tail --search ...`.
export const formatRequestIdLogToken = (requestId: string): string => `oa_req=${requestId}`;

export const getOrCreateRequestId = (request: Request): string => {
  const existing = request.headers.get(OA_REQUEST_ID_HEADER);
  if (existing && existing.length > 0 && existing.length <= 128) return existing;
  // Prefer an ID that already exists upstream when available.
  const cfRay = request.headers.get("cf-ray");
  if (cfRay && cfRay.length > 0 && cfRay.length <= 128) return `cf:${cfRay}`;
  return crypto.randomUUID();
};

export const withRequestIdHeader = (request: Request, requestId: string): Request => {
  const current = request.headers.get(OA_REQUEST_ID_HEADER);
  if (current === requestId) return request;
  const headers = new Headers(request.headers);
  headers.set(OA_REQUEST_ID_HEADER, requestId);
  return new Request(request, { headers });
};

export const withResponseRequestIdHeader = (response: Response, requestId: string): Response => {
  // Avoid touching websocket upgrade responses (if introduced later).
  if (response.status === 101) return response;
  const headers = new Headers(response.headers);
  headers.set(OA_REQUEST_ID_HEADER, requestId);
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
};

