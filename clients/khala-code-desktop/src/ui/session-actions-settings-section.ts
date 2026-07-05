import type {
  KhalaCodeSessionActionIntent,
  KhalaCodeSessionActionProjection,
} from "../shared/session-actions"

export type KhalaCodeSessionActionsSettingsSectionHandle = Readonly<{
  render: () => HTMLElement
  refresh: () => Promise<void>
}>

export type KhalaCodeSessionActionsSettingsSectionOptions = Readonly<{
  fetch: () => Promise<KhalaCodeSessionActionProjection>
  runAction: (action: KhalaCodeSessionActionIntent["action"]) => Promise<{ readonly ok: boolean; readonly message?: string }>
}>

const el = <Tag extends keyof HTMLElementTagNameMap>(
  tag: Tag,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[Tag] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

const actionTitle = (action: KhalaCodeSessionActionIntent["action"]): string => {
  switch (action) {
    case "fork":
      return "Fork Session"
    case "share":
      return "Share Session"
    case "unshare":
      return "Unshare Session"
    case "archive":
      return "Archive Session"
    case "unarchive":
      return "Unarchive Session"
    case "restore_closed_tab":
      return "Restore Closed Session"
    case "previous_session":
      return "Previous Session"
    case "next_session":
      return "Next Session"
    case "previous_message":
      return "Previous Message"
    case "next_message":
      return "Next Message"
  }
}

export const mountKhalaCodeSessionActionsSettingsSection = (
  options: KhalaCodeSessionActionsSettingsSectionOptions,
): KhalaCodeSessionActionsSettingsSectionHandle => {
  let projection: KhalaCodeSessionActionProjection | null = null
  let status = ""
  let sectionNode: HTMLElement | null = null

  const renderIntoCurrentSection = (): void => {
    if (sectionNode !== null) renderInto(sectionNode)
  }

  const refresh = async (): Promise<void> => {
    try {
      projection = await options.fetch()
      status = ""
    } catch (error) {
      status = error instanceof Error ? error.message : String(error)
    }
    renderIntoCurrentSection()
  }

  const run = async (action: KhalaCodeSessionActionIntent["action"]): Promise<void> => {
    try {
      const result = await options.runAction(action)
      status = result.message ?? (result.ok ? "Session action completed." : "Session action was not available.")
      try {
        projection = await options.fetch()
      } catch {
        // Keep the action result visible if the follow-up refresh cannot load.
      }
      renderIntoCurrentSection()
    } catch (error) {
      status = error instanceof Error ? error.message : String(error)
      renderIntoCurrentSection()
    }
  }

  const row = (intent: KhalaCodeSessionActionIntent): HTMLElement => {
    const item = el("div", "khala-session-actions-row")
    item.dataset.action = intent.action
    item.dataset.enabled = String(intent.enabled)
    item.dataset.runtimeBoundary = intent.runtimeBoundary

    const label = el("div", "khala-session-actions-label")
    label.append(
      el("span", "khala-session-actions-title", actionTitle(intent.action)),
      el("span", "khala-session-actions-command", intent.commandId),
      el("span", "khala-session-actions-reason", intent.reason),
    )

    const button = el("button", "khala-session-actions-button", intent.enabled ? "Run" : "Unavailable")
    button.type = "button"
    button.disabled = !intent.enabled
    button.addEventListener("click", () => {
      void run(intent.action)
    })

    item.append(label, button)
    return item
  }

  function renderInto(section: HTMLElement): void {
    section.replaceChildren(el("h3", "khala-settings-section-title", "Session Actions"))
    if (status.length > 0) section.append(el("div", "khala-keybindings-status khala-session-actions-status", status))
    if (projection === null) {
      section.append(el("p", "khala-settings-empty", "Session action state has not been loaded yet."))
      return
    }

    const meta = el("div", "khala-session-actions-meta")
    meta.append(
      el("span", undefined, projection.activeThreadTitle ?? "No active session"),
      el("span", undefined, `${projection.sessionCount} sessions`),
      el("span", undefined, `${projection.messageCount} messages`),
      el("span", undefined, `${projection.closedTabs.length} closed`),
    )

    const list = el("div", "khala-session-actions-list")
    list.append(...projection.intents.map(row))

    section.append(meta, list)
  }

  const render = (): HTMLElement => {
    const section = el("section", "khala-settings-section khala-session-actions-section")
    sectionNode = section
    renderInto(section)
    return section
  }

  return { render, refresh }
}
