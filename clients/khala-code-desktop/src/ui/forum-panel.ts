export type KhalaCodeForumPanelHandle = Readonly<{
  refresh: () => Promise<void>
  setVisible: (visible: boolean) => void
}>

type ForumJson =
  | string
  | number
  | boolean
  | null
  | readonly ForumJson[]
  | { readonly [key: string]: ForumJson }

export type KhalaCodeForumPanelRequest = Readonly<{
  body?: ForumJson
  headers?: Readonly<Record<string, string>>
  method?: "GET" | "POST"
  path: string
}>

export type KhalaCodeForumPanelOptions = Readonly<{
  baseUrl?: string
  fetch?: typeof fetch
  openExternal: (url: string) => Promise<boolean>
  request?: (request: KhalaCodeForumPanelRequest) => Promise<unknown>
}>

type ForumView =
  | { readonly phase: "loading" }
  | { readonly phase: "error"; readonly message: string }
  | { readonly phase: "ready"; readonly board: ForumBoard; readonly launchStatus: ForumLaunchStatus | null }
  | { readonly phase: "forum"; readonly forum: ForumInfo; readonly topics: readonly ForumTopic[] }
  | { readonly phase: "topic"; readonly topic: ForumTopic; readonly posts: readonly ForumPost[]; readonly launchStatus: ForumLaunchStatus | null }
  | { readonly phase: "writeResult"; readonly message: string; readonly topicId?: string }

type ForumBoard = Readonly<{
  forums: readonly ForumInfo[]
}>

type ForumInfo = Readonly<{
  description?: string
  forumId?: string
  locked?: boolean
  postCount?: number
  slug?: string
  title?: string
  topicCount?: number
}>

type ForumTopic = Readonly<{
  author?: ForumActor
  createdAt?: string
  forumId?: string
  postCount?: number
  replyCount?: number
  slug?: string
  state?: string
  title?: string
  topicId?: string
  updatedAt?: string
  viewCount?: number
}>

type ForumPost = Readonly<{
  author?: ForumActor
  bodyText?: string | null
  createdAt?: string
  postId?: string
  postNumber?: number
  subject?: string
  tipRecipientReadiness?: ForumTipReadiness
  tipStats?: ForumTipStats
  topicId?: string
}>

type ForumActor = Readonly<{
  actorRef?: string
  displayName?: string
}>

type ForumTipReadiness = Readonly<{
  blockerRef?: string
  tippingAvailable?: boolean
}>

type ForumTipStats = Readonly<{
  tipCount?: number
  totalPaidSats?: number
  totalSettledSats?: number
}>

type ForumLaunchStatus = Readonly<{
  publicTipping?: {
    readonly postTips?: string
    readonly remainingBeforeLiveTips?: readonly string[]
  }
}>

const ProductPromisesForumRef = "product-promises"
const OpenAgentsBaseUrl = "https://openagents.com"
const DefaultTipSats = 10

const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

const asRecord = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" ? value as Record<string, unknown> : {}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const countText = (
  count: number | undefined,
  singular: string,
  plural: string,
): string => {
  const value = Number.isFinite(count) ? Number(count) : 0
  return value === 1 ? `1 ${singular}` : `${value} ${plural}`
}

const friendlyTime = (value: string | undefined): string => {
  if (value === undefined || value.trim() === "") return "Unknown time"
  const timestamp = Date.parse(value)
  if (!Number.isFinite(timestamp)) return "Unknown time"
  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    month: "short",
  }).format(timestamp)
}

const externalPath = (baseUrl: string, path: string): string =>
  new URL(path, baseUrl).toString()

const idempotencyKey = (kind: string, target: string): string =>
  `khala-code:${kind}:${target}:${Date.now().toString(36)}:${Math.random().toString(36).slice(2, 10)}`

const safeTrimmedText = (value: string, fallback: string): string => {
  const trimmed = value.trim()
  return trimmed.length === 0 ? fallback : trimmed
}

const appendStatus = (root: HTMLElement, text: string): void => {
  const status = root.querySelector<HTMLElement>("[data-khala-forum-status]")
  if (status !== null) status.textContent = text
}

const inputValue = (root: HTMLElement, selector: string): string =>
  root.querySelector<HTMLInputElement | HTMLTextAreaElement>(selector)?.value ?? ""

const setInputValue = (root: HTMLElement, selector: string, value: string): void => {
  const input = root.querySelector<HTMLInputElement | HTMLTextAreaElement>(selector)
  if (input !== null) input.value = value
}

const headersRecord = (headers: HeadersInit | undefined): Readonly<Record<string, string>> => {
  if (headers === undefined) return {}
  if (headers instanceof Headers) return Object.fromEntries(headers.entries())
  if (Array.isArray(headers)) return Object.fromEntries(headers)
  return headers
}

const requestMethod = (method: string | undefined): "GET" | "POST" => {
  if (method === undefined || method === "GET") return "GET"
  if (method === "POST") return "POST"
  throw new Error(`Unsupported Forum method: ${method}`)
}

const requestBody = (body: BodyInit | null | undefined): ForumJson | undefined => {
  if (body === undefined || body === null) return undefined
  if (typeof body !== "string") throw new Error("Forum panel only sends JSON request bodies.")
  return JSON.parse(body) as ForumJson
}

export const mountKhalaCodeForumPanel = (
  container: HTMLElement,
  options: KhalaCodeForumPanelOptions,
): KhalaCodeForumPanelHandle => {
  const baseUrl = options.baseUrl ?? OpenAgentsBaseUrl
  const fetchFn = options.fetch ?? fetch.bind(globalThis)
  let visible = false
  let view: ForumView = { phase: "loading" }
  let activeTopicId: string | null = null
  let loading: Promise<void> | null = null

  const request = async <T>(
    path: string,
    init: RequestInit = {},
  ): Promise<T> => {
    if (options.request !== undefined) {
      const body = requestBody(init.body)
      return await options.request({
        ...(body === undefined ? {} : { body }),
        headers: headersRecord(init.headers),
        method: requestMethod(init.method),
        path,
      }) as T
    }
    const response = await fetchFn(externalPath(baseUrl, path), {
      ...init,
      credentials: "include",
      headers: {
        ...(init.body === undefined ? {} : { "content-type": "application/json" }),
        ...(init.headers ?? {}),
      },
    })
    const payload = await response.json().catch(() => ({})) as unknown
    if (!response.ok) {
      const record = asRecord(payload)
      const reason = typeof record.reason === "string"
        ? record.reason
        : typeof record.error === "string"
          ? record.error
          : "Forum request failed"
      throw new Error(reason)
    }
    return payload as T
  }

  const loadBoard = async (): Promise<void> => {
    view = { phase: "loading" }
    render()
    try {
      const [board, launchStatus] = await Promise.all([
        request<ForumBoard>("/api/forum"),
        request<ForumLaunchStatus>("/api/forum/launch-status").catch(() => null),
      ])
      activeTopicId = null
      view = { phase: "ready", board, launchStatus }
    } catch (error) {
      view = { phase: "error", message: errorMessage(error) }
    }
    render()
  }

  const loadForum = async (forumRef = ProductPromisesForumRef): Promise<void> => {
    view = { phase: "loading" }
    render()
    try {
      const [forum, topics] = await Promise.all([
        request<ForumInfo>(`/api/forum/forums/${encodeURIComponent(forumRef)}`),
        request<{ readonly topics?: readonly ForumTopic[] }>(
          `/api/forum/forums/${encodeURIComponent(forumRef)}/topics`,
        ),
      ])
      activeTopicId = null
      view = { phase: "forum", forum, topics: topics.topics ?? [] }
    } catch (error) {
      view = { phase: "error", message: errorMessage(error) }
    }
    render()
  }

  const loadTopic = async (topicId: string): Promise<void> => {
    view = { phase: "loading" }
    render()
    try {
      const [topic, launchStatus] = await Promise.all([
        request<{ readonly topic: ForumTopic; readonly posts?: readonly ForumPost[] }>(
          `/api/forum/topics/${encodeURIComponent(topicId)}`,
        ),
        request<ForumLaunchStatus>("/api/forum/launch-status").catch(() => null),
      ])
      activeTopicId = topic.topic.topicId ?? topicId
      view = {
        phase: "topic",
        launchStatus,
        posts: topic.posts ?? [],
        topic: topic.topic,
      }
    } catch (error) {
      view = { phase: "error", message: errorMessage(error) }
    }
    render()
  }

  const refresh = async (): Promise<void> => {
    if (loading !== null) return loading
    loading = (activeTopicId === null ? loadForum() : loadTopic(activeTopicId))
      .finally(() => {
        loading = null
      })
    return loading
  }

  const postTopic = async (): Promise<void> => {
    appendStatus(container, "Posting topic...")
    try {
      const title = safeTrimmedText(inputValue(container, "[data-khala-forum-topic-title]"), "Product promise gap")
      const bodyText = safeTrimmedText(
        inputValue(container, "[data-khala-forum-topic-body]"),
        "Product promise gap report from Khala Code.",
      )
      const result = await request<{ readonly topic?: ForumTopic }>(
        `/api/forum/forums/${ProductPromisesForumRef}/topics`,
        {
          body: JSON.stringify({ bodyText, title }),
          headers: { "Idempotency-Key": idempotencyKey("topic", ProductPromisesForumRef) },
          method: "POST",
        },
      )
      const topicId = result.topic?.topicId
      view = {
        phase: "writeResult",
        message: topicId === undefined ? "Topic posted." : "Topic posted to Product Promises.",
        ...(topicId === undefined ? {} : { topicId }),
      }
      setInputValue(container, "[data-khala-forum-topic-title]", "")
      setInputValue(container, "[data-khala-forum-topic-body]", "")
    } catch (error) {
      appendStatus(container, `Post failed: ${errorMessage(error)}`)
      return
    }
    render()
  }

  const postReply = async (): Promise<void> => {
    if (activeTopicId === null) return
    appendStatus(container, "Posting reply...")
    try {
      const bodyText = safeTrimmedText(
        inputValue(container, "[data-khala-forum-reply-body]"),
        "Reply from Khala Code.",
      )
      await request(`/api/forum/topics/${encodeURIComponent(activeTopicId)}/posts`, {
        body: JSON.stringify({ bodyText }),
        headers: { "Idempotency-Key": idempotencyKey("reply", activeTopicId) },
        method: "POST",
      })
      setInputValue(container, "[data-khala-forum-reply-body]", "")
      appendStatus(container, "Reply posted.")
      await loadTopic(activeTopicId)
    } catch (error) {
      appendStatus(container, `Reply failed: ${errorMessage(error)}`)
    }
  }

  const tipPost = async (postId: string): Promise<void> => {
    appendStatus(container, "Sending tip...")
    try {
      const amountRaw = Number(inputValue(container, `[data-khala-forum-tip-amount="${postId}"]`))
      const amountSat = Number.isFinite(amountRaw) && amountRaw > 0 ? Math.trunc(amountRaw) : DefaultTipSats
      const result = await request<{ readonly receiptRef?: string }>(
        `/api/forum/posts/${encodeURIComponent(postId)}/tips/ladder`,
        {
          body: JSON.stringify({ amountSat }),
          headers: { "Idempotency-Key": idempotencyKey("tip", `${postId}:${amountSat}`) },
          method: "POST",
        },
      )
      appendStatus(container, result.receiptRef === undefined ? "Tip submitted." : `Tip recorded: ${result.receiptRef}`)
      if (activeTopicId !== null) await loadTopic(activeTopicId)
    } catch (error) {
      appendStatus(container, `Tip failed: ${errorMessage(error)}`)
    }
  }

  const reportPost = async (postId: string): Promise<void> => {
    appendStatus(container, "Reporting post...")
    try {
      await request(`/api/forum/posts/${encodeURIComponent(postId)}/reports`, {
        body: JSON.stringify({ reason: "off_topic" }),
        headers: { "Idempotency-Key": idempotencyKey("report", postId) },
        method: "POST",
      })
      appendStatus(container, "Report submitted.")
    } catch (error) {
      appendStatus(container, `Report failed: ${errorMessage(error)}`)
    }
  }

  const actionButton = (
    label: string,
    action: string,
    extra?: Readonly<Record<string, string>>,
  ): HTMLButtonElement => {
    const button = el("button", "khala-forum-action", label)
    button.type = "button"
    button.dataset.khalaForumAction = action
    for (const [key, value] of Object.entries(extra ?? {})) {
      button.dataset[key] = value
    }
    return button
  }

  const renderHeader = (): HTMLElement => {
    const header = el("header", "khala-forum-header")
    const titleGroup = el("div", "khala-forum-title-group")
    titleGroup.append(
      el("div", "khala-forum-eyebrow", "OpenAgents Forum"),
      el("h2", "khala-forum-title", "Forum"),
      el("p", "khala-forum-subtitle", "Browse, post, tip, and report through the server Forum authority."),
    )
    const actions = el("div", "khala-forum-actions")
    actions.append(
      actionButton("Forums", "forums"),
      actionButton("Product Promises", "product-promises"),
      actionButton("Refresh", "refresh"),
      actionButton("Open Web Forum", "open-web-forum"),
    )
    header.append(titleGroup, actions)
    return header
  }

  const renderStatus = (): HTMLElement => {
    const status = el("div", "khala-forum-status", "")
    status.dataset.khalaForumStatus = ""
    status.setAttribute("role", "status")
    status.setAttribute("aria-live", "polite")
    return status
  }

  const renderTopicComposer = (): HTMLElement => {
    const form = el("section", "khala-forum-composer")
    const title = Object.assign(el("input", "khala-forum-input") as HTMLInputElement, {
      placeholder: "Short title",
      type: "text",
    })
    title.dataset.khalaForumTopicTitle = ""
    const body = Object.assign(el("textarea", "khala-forum-textarea") as HTMLTextAreaElement, {
      placeholder: "Public-safe report body",
      rows: 4,
    })
    body.dataset.khalaForumTopicBody = ""
    form.append(
      el("h3", "khala-forum-section-title", "Product promise gap"),
      title,
      body,
      actionButton("Post gap report", "post-topic"),
    )
    return form
  }

  const renderBoard = (board: ForumBoard): HTMLElement => {
    const section = el("section", "khala-forum-section")
    section.append(el("h3", "khala-forum-section-title", "Forums"))
    const list = el("div", "khala-forum-list")
    for (const forum of board.forums ?? []) {
      const forumRef = forum.slug ?? forum.forumId ?? ""
      const row = actionButton(forum.title ?? forumRef, "open-forum", { forumRef })
      row.className = "khala-forum-row"
      row.append(
        el("span", "khala-forum-row-summary", `${countText(forum.topicCount, "topic", "topics")} / ${countText(forum.postCount, "post", "posts")}`),
      )
      list.append(row)
    }
    if ((board.forums ?? []).length === 0) {
      list.append(el("div", "khala-forum-empty", "No listed forums returned."))
    }
    section.append(list)
    return section
  }

  const renderForum = (forum: ForumInfo, topics: readonly ForumTopic[]): HTMLElement => {
    const section = el("section", "khala-forum-section")
    const heading = el("div", "khala-forum-section-heading")
    heading.append(
      el("div", undefined, forum.title ?? "Product Promises"),
      el("span", "khala-forum-section-meta", countText(topics.length, "topic", "topics")),
    )
    section.append(heading)
    const list = el("div", "khala-forum-list")
    for (const topic of topics) {
      const topicId = topic.topicId ?? topic.slug ?? ""
      const row = actionButton(topic.title ?? topicId, "open-topic", { topicId })
      row.className = "khala-forum-row"
      row.append(
        el("span", "khala-forum-row-summary", `${topic.author?.displayName ?? "Unknown"} / ${friendlyTime(topic.updatedAt ?? topic.createdAt)} / ${countText(topic.postCount, "post", "posts")}`),
      )
      list.append(row)
    }
    if (topics.length === 0) list.append(el("div", "khala-forum-empty", "No topics returned."))
    section.append(list)
    return section
  }

  const renderTipControls = (post: ForumPost, launchStatus: ForumLaunchStatus | null): HTMLElement => {
    const controls = el("div", "khala-forum-post-actions")
    const postId = post.postId ?? ""
    const stats = post.tipStats
    if (stats !== undefined && Number(stats.totalPaidSats ?? 0) > 0) {
      controls.append(el("span", "khala-forum-tip-total", `${stats.totalPaidSats ?? 0} sats paid`))
    }
    const tippingReady = launchStatus?.publicTipping?.postTips === "ready" &&
      post.tipRecipientReadiness?.tippingAvailable === true
    if (tippingReady) {
      const tipInput = Object.assign(el("input", "khala-forum-tip-input") as HTMLInputElement, {
        inputMode: "numeric",
        min: "1",
        type: "number",
        value: String(DefaultTipSats),
      })
      tipInput.dataset.khalaForumTipAmount = postId
      controls.append(tipInput, actionButton("Tip", "tip-post", { postId }))
    } else {
      controls.append(el("span", "khala-forum-tip-pending", "Tips pending"))
    }
    controls.append(actionButton("Report", "report-post", { postId }))
    return controls
  }

  const renderTopic = (
    topic: ForumTopic,
    posts: readonly ForumPost[],
    launchStatus: ForumLaunchStatus | null,
  ): HTMLElement => {
    const section = el("section", "khala-forum-section")
    const heading = el("div", "khala-forum-section-heading")
    heading.append(
      el("div", undefined, topic.title ?? "Topic"),
      el("span", "khala-forum-section-meta", countText(posts.length, "post", "posts")),
    )
    section.append(heading)
    const postsRoot = el("div", "khala-forum-post-list")
    for (const post of posts) {
      const article = el("article", "khala-forum-post")
      const title = post.subject ?? `Post #${post.postNumber ?? 0}`
      article.append(
        el("div", "khala-forum-post-title", title),
        el("div", "khala-forum-post-meta", `${post.author?.displayName ?? "Unknown"} / ${friendlyTime(post.createdAt)}`),
        el("p", "khala-forum-post-body", post.bodyText ?? "Post body unavailable."),
        renderTipControls(post, launchStatus),
      )
      postsRoot.append(article)
    }
    if (posts.length === 0) postsRoot.append(el("div", "khala-forum-empty", "No visible posts returned."))
    const reply = el("section", "khala-forum-composer")
    const replyBody = Object.assign(el("textarea", "khala-forum-textarea") as HTMLTextAreaElement, {
      placeholder: "Public-safe reply",
      rows: 4,
    })
    replyBody.dataset.khalaForumReplyBody = ""
    reply.append(
      el("h3", "khala-forum-section-title", "Reply"),
      replyBody,
      actionButton("Post reply", "post-reply"),
    )
    section.append(postsRoot, reply)
    return section
  }

  const renderBody = (): HTMLElement => {
    const body = el("div", "khala-forum-body")
    if (view.phase === "loading") {
      body.append(el("div", "khala-forum-empty", "Loading Forum..."))
      return body
    }
    if (view.phase === "error") {
      body.append(
        el("div", "khala-forum-error", view.message),
        actionButton("Open Web Forum", "open-web-forum"),
      )
      return body
    }
    if (view.phase === "ready") {
      body.append(renderTopicComposer(), renderBoard(view.board))
      return body
    }
    if (view.phase === "forum") {
      body.append(renderTopicComposer(), renderForum(view.forum, view.topics))
      return body
    }
    if (view.phase === "topic") {
      body.append(renderTopic(view.topic, view.posts, view.launchStatus))
      return body
    }
    body.append(
      el("div", "khala-forum-success", view.message),
      ...(view.topicId === undefined ? [] : [actionButton("Open topic", "open-topic", { topicId: view.topicId })]),
      actionButton("Product Promises", "product-promises"),
    )
    return body
  }

  function render(): void {
    container.hidden = !visible
    container.replaceChildren(renderHeader(), renderStatus(), renderBody())
  }

  container.addEventListener("click", event => {
    const target = event.target instanceof Element
      ? event.target.closest<HTMLElement>("[data-khala-forum-action]")
      : null
    if (target === null) return
    event.preventDefault()
    const action = target.dataset.khalaForumAction
    if (action === "refresh") void refresh()
    if (action === "forums") void loadBoard()
    if (action === "product-promises") void loadForum(ProductPromisesForumRef)
    if (action === "open-web-forum") void options.openExternal(externalPath(baseUrl, "/forum/f/product-promises"))
    if (action === "open-forum") void loadForum(target.dataset.forumRef ?? ProductPromisesForumRef)
    if (action === "open-topic") {
      const topicId = target.dataset.topicId
      if (topicId !== undefined && topicId !== "") void loadTopic(topicId)
    }
    if (action === "post-topic") void postTopic()
    if (action === "post-reply") void postReply()
    if (action === "tip-post") {
      const postId = target.dataset.postId
      if (postId !== undefined && postId !== "") void tipPost(postId)
    }
    if (action === "report-post") {
      const postId = target.dataset.postId
      if (postId !== undefined && postId !== "") void reportPost(postId)
    }
  })

  render()

  return {
    refresh,
    setVisible: nextVisible => {
      const becameVisible = nextVisible && !visible
      visible = nextVisible
      render()
      if (becameVisible && view.phase === "loading") void refresh()
    },
  }
}
