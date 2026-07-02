import type { Page, Route } from "playwright"
import { makeKhalaCodeQaSeedCorpusFixtureFetch } from "@openagentsinc/khala-qa-harness"

type VisualSmokeRpcOverrideInput = Readonly<{
  args: readonly unknown[]
  method: string
  route: Route
}>

export type KhalaCodeVisualSmokeRpcOverride = (
  input: VisualSmokeRpcOverrideInput,
) => Promise<unknown> | unknown

export type KhalaCodeVisualSmokeRpcMockOptions = Readonly<{
  observedAt?: string
  overrides?: Readonly<Record<string, KhalaCodeVisualSmokeRpcOverride>>
}>

const defaultObservedAt = "2026-07-01T00:00:00.000Z"

export async function installKhalaCodeVisualSmokeRpcMocks(
  page: Page,
  options: KhalaCodeVisualSmokeRpcMockOptions = {},
): Promise<void> {
  const fixtureFetch = makeKhalaCodeQaSeedCorpusFixtureFetch()

  await page.route("**/rpc/*", async route => {
    const method = rpcMethodFromRoute(route)
    if (method === "events") {
      await fulfillEventStream(route, options.observedAt ?? defaultObservedAt)
      return
    }

    const args = await requestArgs(route)
    const override = options.overrides?.[method]
    if (override !== undefined) {
      await fulfillJson(route, await override({ args, method, route }))
      return
    }

    const response = await fixtureFetch(`http://fixture.local/rpc/${encodeURIComponent(method)}`, {
      body: JSON.stringify({ args }),
      headers: { "content-type": "application/json" },
      method: route.request().method(),
    })
    await route.fulfill({
      body: await response.text(),
      contentType: response.headers.get("content-type") ?? "application/json",
      status: response.status,
    })
  })
}

const rpcMethodFromRoute = (route: Route): string =>
  decodeURIComponent(new URL(route.request().url()).pathname.split("/").at(-1) ?? "")

const requestArgs = async (route: Route): Promise<readonly unknown[]> => {
  const postData = route.request().postData()
  if (postData === null || postData.trim() === "") return []
  const parsed = JSON.parse(postData) as { args?: readonly unknown[] }
  return parsed.args ?? []
}

const fulfillJson = async (route: Route, payload: unknown): Promise<void> => {
  await route.fulfill({
    body: JSON.stringify(payload),
    contentType: "application/json",
  })
}

const fulfillEventStream = async (
  route: Route,
  observedAt: string,
): Promise<void> => {
  await route.fulfill({
    body: [
      "event: khala_code_fixture_ready",
      "id: visual-smoke-boot",
      `data: ${JSON.stringify({ observedAt, ok: true })}`,
      "",
      "",
    ].join("\n"),
    contentType: "text/event-stream",
    headers: {
      "cache-control": "no-store",
    },
    status: 200,
  })
}
