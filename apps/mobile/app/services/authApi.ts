import Config from "@/config"

const base = () => Config.authApiUrl.replace(/\/$/, "")

export type AuthStartResult = { ok: true } | { ok: false; error: string }
export type AuthVerifyResult =
  | { ok: true; userId: string; token?: string }
  | { ok: false; error: string }
export type SsoAuthorizeUrlResult = { ok: true; url: string } | { ok: false; error: string }
export type SsoExchangeResult =
  | {
      ok: true
      userId: string
      user: { id: string; email: string | null; firstName: string | null; lastName: string | null }
      token: string
    }
  | { ok: false; error: string }

export async function authStart(email: string): Promise<AuthStartResult> {
  const res = await fetch(`${base()}/api/auth/start`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ email: email.trim().toLowerCase() }),
  })
  const data = (await res.json().catch(() => null)) as { ok?: boolean; error?: string } | null
  if (!res.ok || !data?.ok) {
    return { ok: false, error: typeof data?.error === "string" ? data.error : "send_failed" }
  }
  return { ok: true }
}

export async function authVerify(
  email: string,
  code: string,
): Promise<AuthVerifyResult> {
  const res = await fetch(`${base()}/api/auth/verify`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "X-Client": "openagents-expo",
    },
    body: JSON.stringify({
      email: email.trim().toLowerCase(),
      code: code.replace(/\s+/g, ""),
    }),
  })
  const data = (await res.json().catch(() => null)) as {
    ok?: boolean
    error?: string
    userId?: string
    token?: string
  } | null
  if (!res.ok || !data?.ok) {
    return { ok: false, error: typeof data?.error === "string" ? data.error : "verify_failed" }
  }
  return {
    ok: true,
    userId: data.userId ?? "",
    token: data.token,
  }
}

export async function ssoGetAuthorizeUrl(redirectUri: string): Promise<SsoAuthorizeUrlResult> {
  const url = new URL(`${base()}/api/auth/sso/authorize-url`)
  url.searchParams.set("redirect_uri", redirectUri)
  const res = await fetch(url.toString(), { method: "GET" })
  const data = (await res.json().catch(() => null)) as { ok?: boolean; error?: string; url?: string } | null
  if (!res.ok || !data?.ok || typeof data?.url !== "string") {
    return { ok: false, error: typeof data?.error === "string" ? data.error : "authorize_url_failed" }
  }
  return { ok: true, url: data.url }
}

export async function ssoExchangeCode(code: string): Promise<SsoExchangeResult> {
  const res = await fetch(`${base()}/api/auth/sso/exchange`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ code }),
  })
  const data = (await res.json().catch(() => null)) as {
    ok?: boolean
    error?: string
    userId?: string
    user?: { id: string; email: string | null; firstName: string | null; lastName: string | null }
    token?: string
  } | null
  if (!res.ok || !data?.ok || !data?.userId || !data?.user) {
    return { ok: false, error: typeof data?.error === "string" ? data.error : "exchange_failed" }
  }
  return {
    ok: true,
    userId: data.userId,
    user: data.user,
    token: data.token ?? "",
  }
}
