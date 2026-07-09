
import { SARAH_PROSPECT_COOKIE, readProspectRef } from "./realtime-token-guard";

export function threadIdForProspectRef(prospectRef: string) {
  return `prospect:${prospectRef}`;
}

export function readSarahProspectRef(request: Request) {
  return readProspectRef(request);
}

export function mintSarahProspectRef() {
  return crypto.randomUUID();
}

export function setSarahProspectCookie(
  response: Response,
  prospectRef: string,
) {
  response.headers.append(
    "set-cookie",
    [
      `${SARAH_PROSPECT_COOKIE}=${encodeURIComponent(prospectRef)}`,
      "Path=/",
      "HttpOnly",
      "SameSite=Lax",
      "Max-Age=31536000",
      process.env.NODE_ENV === "production" ? "Secure" : null,
    ]
      .filter(Boolean)
      .join("; "),
  );
}
