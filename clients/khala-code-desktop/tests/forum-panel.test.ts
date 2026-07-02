import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { Window } from "happy-dom"

import { mountKhalaCodeForumPanel } from "../src/ui/forum-panel"

const setGlobal = (key: string, value: unknown): void => {
  Object.defineProperty(globalThis, key, {
    configurable: true,
    value,
    writable: true,
  })
}

const installDom = (): HTMLElement => {
  const window = new Window()
  setGlobal("document", window.document)
  setGlobal("HTMLElement", window.HTMLElement)
  setGlobal("HTMLInputElement", window.HTMLInputElement)
  setGlobal("HTMLTextAreaElement", window.HTMLTextAreaElement)
  setGlobal("Element", window.Element)
  setGlobal("MouseEvent", window.MouseEvent)
  setGlobal("customElements", window.customElements)
  const container = window.document.createElement("section")
  window.document.body.append(container)
  return container as unknown as HTMLElement
}

const flushForumPanel = async (): Promise<void> => {
  for (let index = 0; index < 50; index += 1) {
    await Promise.resolve()
  }
}

describe("khala code forum panel", () => {
  test("loads through the desktop host Forum transport without renderer fetch", async () => {
    const container = installDom()
    const requests: unknown[] = []
    const panel = mountKhalaCodeForumPanel(container, {
      openExternal: async () => true,
      request: async request => {
        requests.push(request)
        if (request.path === "/api/forum/forums/product-promises") {
          return { forumId: "forum.product-promises", slug: "product-promises", title: "Product Promises" }
        }
        if (request.path === "/api/forum/forums/product-promises/topics") {
          return { topics: [{ topicId: "topic.host", title: "Loaded by host transport" }] }
        }
        throw new Error(`unexpected ${request.path}`)
      },
    })

    panel.setVisible(true)
    await panel.refresh()

    expect(container.dataset.forumShell).toBe("")
    expect(container.querySelector(".khala-forum-panel")).not.toBeNull()
    expect(container.querySelector(".khala-forum-list-header")).not.toBeNull()
    expect(container.querySelector(".khala-forum-index")).not.toBeNull()
    expect(container.textContent).toContain("Loaded by host transport")
    expect(requests).toEqual([
      {
        headers: {},
        method: "GET",
        path: "/api/forum/forums/product-promises",
      },
      {
        headers: {},
        method: "GET",
        path: "/api/forum/forums/product-promises/topics",
      },
    ])
  })

  test("browses, posts, replies, tips, and reports through OpenAgents Forum routes", async () => {
    const container = installDom()
    const requests: Array<{
      readonly body: string | undefined
      readonly credentials: RequestCredentials | undefined
      readonly headers: HeadersInit | undefined
      readonly method: string | undefined
      readonly url: string
    }> = []
    const fetchStub = Object.assign(async (input: RequestInfo | URL, init: RequestInit = {}) => {
      const url = String(input)
      requests.push({
        body: typeof init.body === "string" ? init.body : undefined,
        credentials: init.credentials,
        headers: init.headers,
        method: init.method,
        url,
      })

      const payload = url.endsWith("/api/forum/forums/product-promises")
        ? { forumId: "forum.product-promises", slug: "product-promises", title: "Product Promises" }
        : url.endsWith("/api/forum/forums/product-promises/topics")
          ? init.method === "POST"
            ? { topic: { topicId: "topic.new", title: "New gap" } }
            : { topics: [{ topicId: "topic.1", title: "Forum parity", postCount: 1, author: { displayName: "Owner" } }] }
          : url.endsWith("/api/forum/topics/topic.1")
            ? {
                posts: [
                  {
                    author: { displayName: "Owner" },
                    bodyText: "Forum slot must match web forum.",
                    postId: "post.1",
                    postNumber: 1,
                    subject: "Forum parity",
                    tipRecipientReadiness: { tippingAvailable: true },
                    tipStats: { totalPaidSats: 0 },
                  },
                ],
                topic: { topicId: "topic.1", title: "Forum parity" },
              }
            : url.endsWith("/api/forum/launch-status")
              ? { publicTipping: { postTips: "ready", remainingBeforeLiveTips: [] } }
              : url.endsWith("/api/forum/topics/topic.1/posts")
                ? { post: { postId: "post.2" } }
                : url.endsWith("/api/forum/posts/post.1/tips/ladder")
                  ? { receiptRef: "receipt.tip.1" }
                  : url.endsWith("/api/forum/posts/post.1/reports")
                    ? { reportId: "report.1" }
                    : {}

      return new Response(JSON.stringify(payload), {
        headers: { "content-type": "application/json" },
        status: 200,
      })
    }, { preconnect: () => {} }) as typeof fetch

    const opened: string[] = []
    const panel = mountKhalaCodeForumPanel(container, {
      baseUrl: "https://openagents.test",
      fetch: fetchStub,
      openExternal: async url => {
        opened.push(url)
        return true
      },
    })

    panel.setVisible(true)
    await panel.refresh()

    expect(container.hidden).toBe(false)
    expect(container.textContent).toContain("Product Promises")
    expect(container.textContent).toContain("Forum parity")
    expect(requests.at(-2)?.url).toBe("https://openagents.test/api/forum/forums/product-promises")
    expect(requests.at(-1)?.url).toBe("https://openagents.test/api/forum/forums/product-promises/topics")
    expect(requests.every(request => request.credentials === "include")).toBe(true)

    const topicButton = container.querySelector<HTMLButtonElement>("[data-topic-id='topic.1']")
    expect(topicButton).not.toBeNull()
    topicButton?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushForumPanel()
    expect(requests.some(request => request.url === "https://openagents.test/api/forum/topics/topic.1")).toBe(true)
    expect(container.textContent).toContain("Forum slot must match web forum.")
    expect(container.querySelector(".khala-forum-author-rail")).not.toBeNull()
    expect(container.querySelector(".khala-forum-post-content")).not.toBeNull()
    expect(container.querySelector(".khala-forum-tip-label")).not.toBeNull()

    container.querySelector<HTMLButtonElement>("[data-khala-forum-action='report-post']")
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushForumPanel()
    const reportRequest = requests.find(request => request.url.endsWith("/api/forum/posts/post.1/reports"))
    expect(reportRequest?.method).toBe("POST")
    expect(reportRequest?.body).toBe(JSON.stringify({ reason: "off_topic" }))

    const reply = container.querySelector<HTMLTextAreaElement>("[data-khala-forum-reply-body]")
    expect(reply).not.toBeNull()
    if (reply !== null) reply.value = "Desktop parity reply."
    container.querySelector<HTMLButtonElement>("[data-khala-forum-action='post-reply']")
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushForumPanel()
    const replyRequest = requests.find(request => request.url.endsWith("/api/forum/topics/topic.1/posts"))
    expect(replyRequest?.method).toBe("POST")
    expect(replyRequest?.body).toBe(JSON.stringify({ bodyText: "Desktop parity reply." }))

    const tipInput = container.querySelector<HTMLInputElement>("[data-khala-forum-tip-amount='post.1']")
    expect(tipInput).not.toBeNull()
    if (tipInput !== null) tipInput.value = "21"
    container.querySelector<HTMLButtonElement>("[data-khala-forum-action='tip-post']")
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushForumPanel()
    const tipRequest = requests.find(request => request.url.endsWith("/api/forum/posts/post.1/tips/ladder"))
    expect(tipRequest?.method).toBe("POST")
    expect(tipRequest?.body).toBe(JSON.stringify({ amountSat: 21 }))

    container.querySelector<HTMLButtonElement>("[data-khala-forum-action='product-promises']")
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushForumPanel()
    const title = container.querySelector<HTMLInputElement>("[data-khala-forum-topic-title]")
    const body = container.querySelector<HTMLTextAreaElement>("[data-khala-forum-topic-body]")
    expect(title).not.toBeNull()
    expect(body).not.toBeNull()
    if (title !== null) title.value = "Desktop Forum gap"
    if (body !== null) body.value = "Forum in Khala Code is now being checked."
    container.querySelector<HTMLButtonElement>("[data-khala-forum-action='post-topic']")
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushForumPanel()
    const topicPostRequest = [...requests].reverse().find(request =>
      request.url.endsWith("/api/forum/forums/product-promises/topics") && request.method === "POST"
    )
    expect(topicPostRequest?.body).toBe(JSON.stringify({
      bodyText: "Forum in Khala Code is now being checked.",
      title: "Desktop Forum gap",
    }))

    container.querySelector<HTMLButtonElement>("[data-khala-forum-action='open-web-forum']")
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    expect(opened).toEqual(["https://openagents.test/forum/f/product-promises"])
  })

  test("keeps the desktop Forum on the website Khala Forum surface tokens", () => {
    const styles = readFileSync(new URL("../src/ui/styles.css", import.meta.url), "utf8")
    const forumBlockStart = styles.indexOf(".khala-code-forum {")
    const forumBlockEnd = styles.indexOf("/* Fleet status panel")
    const forumBlock = styles.slice(forumBlockStart, forumBlockEnd)

    expect(forumBlock).toContain("--khala-forum-heading: var(--oa-color-khala-text-bright);")
    expect(forumBlock).toContain("--khala-forum-header: var(--oa-color-khala-surface-active);")
    expect(forumBlock).toContain("--khala-forum-link: var(--oa-color-khala-energy-soft);")
    expect(forumBlock).toContain("--khala-forum-link-hover: var(--oa-color-khala-energy-cyan);")
    expect(forumBlock).toContain("--khala-forum-navbar: var(--oa-color-khala-surface-muted);")
    expect(forumBlock).toContain("--khala-forum-page: var(--oa-color-khala-surface);")
    expect(forumBlock).toContain("--khala-forum-panel: var(--oa-color-khala-surface-raised);")
    expect(forumBlock).toContain("--khala-forum-payment: var(--oa-color-khala-energy-line);")
    expect(forumBlock).toContain("--khala-forum-post-link: var(--oa-color-khala-energy-blue);")
    expect(forumBlock).toContain("--khala-forum-post-link-hover-bg: var(--oa-color-khala-surface-active);")
    expect(forumBlock).toContain("--khala-forum-row-a: var(--oa-color-khala-surface);")
    expect(forumBlock).toContain("--khala-forum-row-b: var(--oa-color-khala-surface-muted);")
    expect(forumBlock).toContain("--khala-forum-row-c: var(--oa-color-khala-border);")
    expect(forumBlock).toContain("--khala-forum-text: var(--oa-color-khala-text-muted);")
    expect(forumBlock).toContain("--khala-forum-wrap: var(--oa-color-khala-surface);")
    expect(forumBlock).toContain("--khala-forum-wrap-border: var(--oa-color-khala-border);")
    expect(forumBlock).toContain("background: var(--khala-forum-page);")
    expect(forumBlock).toContain("grid-template-columns: 2.5rem minmax(0, 1fr) 5.5rem 5.5rem")
    expect(forumBlock).not.toContain("var(--oa-color-component-surface)")
    expect(forumBlock).not.toContain("var(--oa-color-component-text)")
  })
})
