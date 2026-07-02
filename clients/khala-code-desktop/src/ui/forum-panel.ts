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
  discoverability?: string
  forumId?: string
  lastPost?: ForumLatestPost
  lastPostSummary?: ForumLatestPost
  latestPost?: ForumLatestPost
  locked?: boolean
  postCount?: number
  slug?: string
  summary?: string
  title?: string
  topicCount?: number
}>

type ForumTopic = Readonly<{
  announcement?: boolean
  author?: ForumActor
  createdAt?: string
  forumId?: string
  lastPost?: ForumLatestPost
  lastPostSummary?: ForumLatestPost
  latestPost?: ForumLatestPost
  locked?: boolean
  postCount?: number
  replyCount?: number
  slug?: string
  state?: string
  sticky?: boolean
  title?: string
  topicType?: string
  topicId?: string
  updatedAt?: string
  viewCount?: number
  views?: number
}>

type ForumPost = Readonly<{
  author?: ForumActor
  authorFirstSeenAt?: string
  authorPostCount?: number
  bodyText?: string | null
  contentRef?: string | null
  createdAt?: string
  postId?: string
  postNumber?: number
  subject?: string
  title?: string
  tipRecipientReadiness?: ForumTipReadiness
  tipStats?: ForumTipStats
  topicId?: string
}>

type ForumActor = Readonly<{
  actorId?: string
  actorRef?: string
  displayName?: string
  firstSeenAt?: string
  forumPostCount?: number
  joinedAt?: string
  kind?: string
  postCount?: number
  rank?: string
  role?: string
  slug?: string
}>

type ForumLatestPost = Readonly<{
  actorRef?: string
  author?: ForumActor
  authorDisplayName?: string
  createdAt?: string
  subject?: string
  timestamp?: string
  title?: string
  topicTitle?: string
  updatedAt?: string
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

const numericCount = (count: number | undefined): number =>
  Number.isFinite(count) ? Number(count) : 0

const actorDisplayName = (actor: ForumActor | undefined): string =>
  actor?.displayName ?? actor?.actorRef ?? "Unknown"

const actorInitial = (actor: ForumActor | undefined): string =>
  actorDisplayName(actor).trim().slice(0, 1).toUpperCase() || "A"

const actorRole = (actor: ForumActor | undefined): string =>
  actor?.role ?? actor?.rank ?? actor?.kind ?? "Member"

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

const forumStatusLabel = (forum: ForumInfo): string =>
  forum.locked === true
    ? "Locked forum"
    : forum.discoverability === "unlisted"
      ? "Unlisted forum"
      : "Listed forum"

const topicStatusLabel = (topic: ForumTopic): string =>
  topic.locked === true || topic.state === "locked"
    ? "Locked topic"
    : topic.topicType === "sticky" || topic.sticky === true
      ? "Sticky topic"
      : topic.topicType === "announcement" || topic.announcement === true
        ? "Announcement topic"
        : "Topic"

const latestPostProjection = (
  item: Readonly<{
    lastPost?: ForumLatestPost
    lastPostSummary?: ForumLatestPost
    latestPost?: ForumLatestPost
  }>,
): ForumLatestPost | null => item.lastPost ?? item.lastPostSummary ?? item.latestPost ?? null

const lastPostSubject = (latestPost: ForumLatestPost): string =>
  latestPost.subject ?? latestPost.title ?? latestPost.topicTitle ?? "Last post"

const truncatedSubject = (subject: string): string =>
  subject.length > 48 ? `${subject.slice(0, 47).trimEnd()}...` : subject

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

  const breadcrumbButton = (label: string, action: string): HTMLButtonElement => {
    const button = actionButton(label, action)
    button.className = "khala-forum-breadcrumb-link"
    return button
  }

  const statusMarker = (label: string): HTMLElement => {
    const marker = el("span", "khala-forum-index")
    marker.title = label
    marker.setAttribute("aria-label", label)
    marker.append(el("span"), el("span"), el("span"))
    return marker
  }

  const listHeader = (
    first: string,
    countA: string,
    countB: string,
    last: string,
  ): HTMLElement => {
    const header = el("div", "khala-forum-list-header")
    header.append(
      el("span"),
      el("span", undefined, first),
      el("span", "khala-forum-list-header-number", countA),
      el("span", "khala-forum-list-header-number", countB),
      el("span", undefined, last),
    )
    return header
  }

  const compactMeta = (items: readonly string[]): HTMLElement => {
    const meta = el("span", "khala-forum-row-compact-meta")
    for (const item of items) {
      meta.append(el("span", "khala-forum-row-chip", item))
    }
    return meta
  }

  const lastPostCell = (
    item: Readonly<{
      lastPost?: ForumLatestPost
      lastPostSummary?: ForumLatestPost
      latestPost?: ForumLatestPost
    }>,
  ): HTMLElement => {
    const cell = el("span", "khala-forum-last-post")
    const latestPost = latestPostProjection(item)
    if (latestPost === null) {
      cell.append(el("span", "khala-forum-last-post-empty", "No posts"))
      return cell
    }
    const subject = lastPostSubject(latestPost)
    const author =
      latestPost.author?.displayName ??
      latestPost.author?.actorRef ??
      latestPost.authorDisplayName ??
      latestPost.actorRef ??
      "Unknown"
    const time = friendlyTime(latestPost.createdAt ?? latestPost.updatedAt ?? latestPost.timestamp)
    const subjectNode = el("span", "khala-forum-last-post-title", truncatedSubject(subject))
    subjectNode.title = subject
    cell.append(
      subjectNode,
      el("span", "khala-forum-last-post-meta", `by ${author} / ${time}`),
    )
    return cell
  }

  const renderHeader = (): HTMLElement => {
    const header = el("header", "khala-forum-header")
    const breadcrumb = el("nav", "khala-forum-breadcrumb")
    breadcrumb.setAttribute("aria-label", "Forum breadcrumbs")
    breadcrumb.append(
      breadcrumbButton("Board index", "forums"),
      el("span", "khala-forum-breadcrumb-separator", "/"),
      el("span", "khala-forum-breadcrumb-current", "Khala Code"),
    )

    const masthead = el("section", "khala-forum-masthead khala-forum-panel")
    masthead.append(el("div", "khala-forum-panel-bar", "OpenAgents Forum"))
    const mastheadBody = el("div", "khala-forum-masthead-body")
    const titleGroup = el("div", "khala-forum-title-group")
    titleGroup.append(
      el("div", "khala-forum-eyebrow", "Forum"),
      el("h2", "khala-forum-title", "OpenAgents Forum"),
      el(
        "p",
        "khala-forum-subtitle",
        "Browse, post, tip, and report through the same Forum authority as openagents.com.",
      ),
    )
    const actions = el("div", "khala-forum-actions")
    actions.append(
      actionButton("Forums", "forums"),
      actionButton("Product Promises", "product-promises"),
      actionButton("Refresh", "refresh"),
      actionButton("Open Web Forum", "open-web-forum"),
    )
    mastheadBody.append(titleGroup, actions)
    masthead.append(mastheadBody)
    header.append(breadcrumb, masthead)
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
    const form = el("section", "khala-forum-composer khala-forum-panel")
    form.append(el("div", "khala-forum-panel-bar", "Product promises"))
    const bodyRoot = el("div", "khala-forum-composer-body")
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
    bodyRoot.append(
      el("h3", "khala-forum-section-title", "Product promise gap"),
      title,
      body,
      actionButton("Post gap report", "post-topic"),
    )
    form.append(bodyRoot)
    return form
  }

  const renderBoard = (board: ForumBoard): HTMLElement => {
    const section = el("section", "khala-forum-section khala-forum-panel")
    section.append(el("div", "khala-forum-panel-bar", "OpenAgents Forum"))
    const list = el("div", "khala-forum-list")
    list.append(listHeader("Forum", "Topics", "Posts", "Last post"))
    for (const [index, forum] of (board.forums ?? []).entries()) {
      const forumRef = forum.slug ?? forum.forumId ?? ""
      const topicCount = numericCount(forum.topicCount)
      const postCount = numericCount(forum.postCount)
      const row = actionButton("", "open-forum", { forumRef })
      row.className = "khala-forum-row"
      row.dataset.khalaForumRowTone = index % 2 === 0 ? "a" : "b"
      const main = el("span", "khala-forum-row-main")
      main.append(
        el("span", "khala-forum-row-title", forum.title ?? forumRef),
        el("span", "khala-forum-row-summary", forum.description ?? forum.summary ?? forumStatusLabel(forum)),
        compactMeta([
          countText(topicCount, "topic", "topics"),
          countText(postCount, "post", "posts"),
          latestPostProjection(forum) === null ? "Last post: No posts" : "Last post available",
        ]),
      )
      row.append(
        statusMarker(forumStatusLabel(forum)),
        main,
        el("span", "khala-forum-row-count", String(topicCount)),
        el("span", "khala-forum-row-count", String(postCount)),
        lastPostCell(forum),
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
    const section = el("section", "khala-forum-section khala-forum-panel")
    section.append(el("div", "khala-forum-panel-bar", "Forum"))
    const intro = el("div", "khala-forum-section-intro")
    const introText = el("div", "khala-forum-title-group")
    introText.append(
      el("div", "khala-forum-eyebrow", "Forum"),
      el("h3", "khala-forum-section-title", forum.title ?? "Product Promises"),
      el(
        "p",
        "khala-forum-subtitle",
        `${countText(forum.topicCount, "topic", "topics")} / ${countText(forum.postCount, "post", "posts")}${forum.locked === true ? " / Locked" : ""}`,
      ),
    )
    const badges = el("div", "khala-forum-actions")
    badges.append(
      el(
        "span",
        forum.discoverability === "unlisted"
          ? "khala-forum-badge khala-forum-badge-payment"
          : "khala-forum-badge",
        forum.discoverability === "unlisted" ? "Unlisted" : "Listed",
      ),
    )
    intro.append(introText, badges)
    section.append(intro)
    const list = el("div", "khala-forum-list")
    list.append(listHeader("Topics", "Replies", "Views", "Last post"))
    for (const [index, topic] of topics.entries()) {
      const topicId = topic.topicId ?? topic.slug ?? ""
      const postCount = numericCount(topic.postCount)
      const replyCount = numericCount(topic.replyCount ?? Math.max(postCount - 1, 0))
      const viewCount = numericCount(topic.viewCount ?? topic.views)
      const row = actionButton("", "open-topic", { topicId })
      row.className = "khala-forum-row"
      row.dataset.khalaForumRowTone = index % 2 === 0 ? "a" : "b"
      const main = el("span", "khala-forum-row-main")
      main.append(
        el("span", "khala-forum-row-title", topic.title ?? topicId),
        el(
          "span",
          "khala-forum-row-summary",
          `by ${actorDisplayName(topic.author)} / ${friendlyTime(topic.createdAt ?? topic.updatedAt)}`,
        ),
        compactMeta([
          countText(replyCount, "reply", "replies"),
          countText(viewCount, "view", "views"),
          latestPostProjection(topic) === null ? `Last post: ${friendlyTime(topic.updatedAt)}` : "Last post available",
        ]),
      )
      row.append(
        statusMarker(topicStatusLabel(topic)),
        main,
        el("span", "khala-forum-row-count", String(replyCount)),
        el("span", "khala-forum-row-count", String(viewCount)),
        lastPostCell(topic),
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
      const tipLabel = el("label", "khala-forum-tip-label")
      const tipInput = Object.assign(el("input", "khala-forum-tip-input") as HTMLInputElement, {
        inputMode: "numeric",
        min: "1",
        step: "1",
        type: "number",
        value: String(DefaultTipSats),
      })
      tipInput.dataset.khalaForumTipAmount = postId
      tipLabel.append(el("span", undefined, "Tip"), tipInput, el("span", undefined, "sats"))
      controls.append(tipLabel, actionButton("Send tip", "tip-post", { postId }))
    } else {
      controls.append(el("span", "khala-forum-tip-pending", "Tips pending"))
    }
    controls.append(actionButton("Report", "report-post", { postId }))
    return controls
  }

  const renderAuthorProfile = (post: ForumPost): HTMLElement => {
    const actor = post.author
    const rail = el("aside", "khala-forum-author-rail")
    const identity = el("div", "khala-forum-author-identity")
    identity.append(
      el("span", "khala-forum-author-avatar khala-forum-index", actorInitial(actor)),
      (() => {
        const copy = el("span", "khala-forum-author-copy")
        copy.append(
          el("span", "khala-forum-author-name", actorDisplayName(actor)),
          el("span", "khala-forum-author-role", actorRole(actor)),
        )
        return copy
      })(),
    )
    const postCount = actor?.postCount ?? actor?.forumPostCount ?? post.authorPostCount
    const joinedAt = actor?.joinedAt ?? actor?.firstSeenAt ?? post.authorFirstSeenAt
    const readiness =
      post.tipRecipientReadiness?.tippingAvailable === true
        ? "Wallet ready"
        : post.tipRecipientReadiness === undefined
          ? ""
          : "Wallet pending"
    const details = el("dl", "khala-forum-author-details")
    if (postCount !== undefined) {
      const row = el("div")
      row.append(el("dt", undefined, "Posts:"), el("dd", undefined, String(postCount)))
      details.append(row)
    }
    if (joinedAt !== undefined) {
      const row = el("div")
      row.append(el("dt", undefined, "Joined:"), el("dd", undefined, friendlyTime(joinedAt)))
      details.append(row)
    }
    if (readiness !== "") {
      const row = el("div")
      row.append(el("dt", undefined, "Tips:"), el("dd", undefined, readiness))
      details.append(row)
    }
    rail.append(identity, details)
    return rail
  }

  const renderPostContent = (
    topic: ForumTopic,
    post: ForumPost,
    launchStatus: ForumLaunchStatus | null,
  ): HTMLElement => {
    const content = el("div", "khala-forum-post-content")
    const header = el("header", "khala-forum-post-header")
    const postNumber = numericCount(post.postNumber)
    const title = post.subject ?? post.title ?? topic.title ?? `Post #${postNumber}`
    const titleGroup = el("div", "khala-forum-post-title-group")
    titleGroup.append(
      el("h3", "khala-forum-post-title", title),
      el("p", "khala-forum-post-meta", `Post #${postNumber} / ${friendlyTime(post.createdAt)}`),
    )
    header.append(titleGroup, renderTipControls(post, launchStatus))
    const body = el("div", "khala-forum-post-body", post.bodyText ?? post.contentRef ?? "Post body unavailable.")
    content.append(header, body)
    return content
  }

  const renderTopic = (
    topic: ForumTopic,
    posts: readonly ForumPost[],
    launchStatus: ForumLaunchStatus | null,
  ): HTMLElement => {
    const section = el("section", "khala-forum-section khala-forum-panel")
    section.append(el("div", "khala-forum-panel-bar", "Topic"))
    const intro = el("div", "khala-forum-section-intro")
    const introText = el("div", "khala-forum-title-group")
    introText.append(
      el("div", "khala-forum-eyebrow", "Thread"),
      el("h3", "khala-forum-section-title", topic.title ?? "Topic"),
      el("p", "khala-forum-subtitle", countText(topic.postCount ?? posts.length, "post", "posts")),
    )
    const topicActions = el("div", "khala-forum-actions")
    topicActions.append(actionButton("Forum", "product-promises"))
    intro.append(introText, topicActions)
    section.append(intro)
    const postsRoot = el("div", "khala-forum-post-list")
    for (const [index, post] of posts.entries()) {
      const article = el("article", "khala-forum-post")
      article.dataset.khalaForumRowTone = index % 2 === 0 ? "a" : "b"
      article.append(renderAuthorProfile(post), renderPostContent(topic, post, launchStatus))
      postsRoot.append(article)
    }
    if (posts.length === 0) postsRoot.append(el("div", "khala-forum-empty", "No visible posts returned."))
    const reply = el("section", "khala-forum-composer khala-forum-reply-composer")
    reply.append(el("div", "khala-forum-panel-bar", "Reply"))
    const replyBodyRoot = el("div", "khala-forum-composer-body")
    const replyBody = Object.assign(el("textarea", "khala-forum-textarea") as HTMLTextAreaElement, {
      placeholder: "Public-safe reply",
      rows: 4,
    })
    replyBody.dataset.khalaForumReplyBody = ""
    replyBodyRoot.append(
      el("h3", "khala-forum-section-title", "Reply"),
      replyBody,
      actionButton("Post reply", "post-reply"),
    )
    reply.append(replyBodyRoot)
    section.append(postsRoot, reply)
    return section
  }

  const renderBody = (): HTMLElement => {
    const body = el("div", "khala-forum-body")
    if (view.phase === "loading") {
      body.append(
        el("div", "khala-forum-empty khala-forum-panel", "Loading Forum..."),
      )
      return body
    }
    if (view.phase === "error") {
      const error = el("section", "khala-forum-message khala-forum-panel")
      error.append(
        el("div", "khala-forum-eyebrow", "Forum unavailable"),
        el("div", "khala-forum-error", view.message),
        actionButton("Open Web Forum", "open-web-forum"),
      )
      body.append(error)
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
    const result = el("section", "khala-forum-message khala-forum-panel")
    result.append(
      el("div", "khala-forum-success", view.message),
      ...(view.topicId === undefined
        ? []
        : [actionButton("Open topic", "open-topic", { topicId: view.topicId })]),
      actionButton("Product Promises", "product-promises"),
    )
    body.append(result)
    return body
  }

  function render(): void {
    container.dataset.forumShell = ""
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
    if (action === "open-web-forum") {
      void options.openExternal(externalPath(baseUrl, "/forum/f/product-promises"))
    }
    if (action === "open-forum") void loadForum(target.dataset.forumRef || ProductPromisesForumRef)
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
