import { createAuthService } from "@workos/authkit-session"

import { WebCookieSessionStorage } from "../auth/sessionCookieStorage"
import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId"

const authkit = createAuthService<Request, Response>({
  sessionStorageFactory: (config) => new WebCookieSessionStorage(config),
})

export const handleCallbackRequest = async (request: Request): Promise<Response> => {
  if (request.method !== "GET") {
    return new Response("Method not allowed", { status: 405 })
  }

  const url = new URL(request.url)
  const code = url.searchParams.get("code")
  const state = url.searchParams.get("state") ?? undefined
  if (!code) {
    return new Response("Missing code", { status: 400 })
  }

  try {
    const result = await authkit.handleCallback(request, new Response(null), { code, state })
    const location = new URL(result.returnPathname ?? "/", url).toString()

    const headers = new Headers()
    headers.set("location", location)

    // Persist WorkOS session cookie.
    const setCookie = (result.headers as unknown as Record<string, string>)["Set-Cookie"];
    if (typeof setCookie === "string") {
      headers.append("Set-Cookie", setCookie)
    }

    return new Response(null, { status: 302, headers })
  } catch (err) {
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing"
    console.error(`[auth.callback] ${formatRequestIdLogToken(requestId)}`, err)
    return new Response("Callback failed", { status: 500 })
  }
}
