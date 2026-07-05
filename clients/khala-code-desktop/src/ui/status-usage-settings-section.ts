import type { KhalaCodeStatusUsageProjection } from "../shared/status-usage"

export type KhalaCodeStatusUsageSettingsSectionHandle = Readonly<{
  render: () => HTMLElement
  refresh: () => Promise<void>
}>

export type KhalaCodeStatusUsageSettingsSectionOptions = Readonly<{
  fetch: () => Promise<KhalaCodeStatusUsageProjection>
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

const compactNumber = new Intl.NumberFormat("en-US")

const stateText = (value: string): string => value.replace(/_/g, " ")

export const mountKhalaCodeStatusUsageSettingsSection = (
  options: KhalaCodeStatusUsageSettingsSectionOptions,
): KhalaCodeStatusUsageSettingsSectionHandle => {
  let projection: KhalaCodeStatusUsageProjection | null = null
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

  const metric = (label: string, value: string | number): HTMLElement => {
    const row = el("div", "khala-status-usage-metric")
    row.append(
      el("span", "khala-status-usage-label", label),
      el("span", "khala-status-usage-value", typeof value === "number" ? compactNumber.format(value) : value),
    )
    return row
  }

  function renderInto(section: HTMLElement): void {
    section.replaceChildren(el("h3", "khala-settings-section-title", "Status, Errors, Usage"))
    if (status.length > 0) section.append(el("div", "khala-keybindings-status khala-status-usage-status", status))
    if (projection === null) {
      section.append(el("p", "khala-settings-empty", "Status and usage state has not been loaded yet."))
      return
    }

    const timeline = el("div", "khala-status-usage-grid")
    timeline.append(
      metric("Messages", projection.timeline.messageCount),
      metric("Tool calls", projection.timeline.toolCallCount),
      metric("Anchors", projection.timeline.anchorIds.length),
      metric("Virtualization", projection.timeline.estimatedVirtualizationUseful ? "useful" : "not needed"),
    )

    const usage = el("div", "khala-status-usage-grid")
    usage.dataset.status = projection.usage.status
    usage.append(
      metric("Total tokens", projection.usage.totalTokens),
      metric("Synced", projection.usage.leaderboardSyncedTokens),
      metric("Pending", projection.usage.pendingSyncTokens),
      metric("Missing usage", projection.usage.missingUsageTurns),
    )

    const runtime = el("div", "khala-status-usage-list")
    for (const row of projection.runtime.rows) {
      const item = el("div", "khala-status-usage-row")
      item.dataset.state = row.state
      item.append(
        el("span", "khala-status-usage-row-title", row.label),
        el("span", "khala-status-usage-row-state", row.state),
        el("span", "khala-status-usage-row-detail", row.detail),
      )
      runtime.append(item)
    }
    if (projection.runtime.rows.length === 0) {
      runtime.append(el("p", "khala-settings-empty", "No runtime status rows yet."))
    }

    const errors = el("div", "khala-status-usage-list")
    for (const error of projection.errors) {
      const item = el("div", "khala-status-usage-row")
      item.dataset.state = error.kind
      item.append(
        el("span", "khala-status-usage-row-title", error.title),
        el("span", "khala-status-usage-row-state", error.retryable ? "retryable" : "final"),
        el(
          "span",
          "khala-status-usage-row-detail",
          `${error.detail}${error.settingsEntryPoint === null ? "" : ` / ${stateText(error.settingsEntryPoint)}`}`,
        ),
      )
      errors.append(item)
    }
    if (projection.errors.length === 0) {
      errors.append(el("p", "khala-settings-empty", "No provider errors projected."))
    }

    section.append(
      el("div", "khala-status-usage-group-title", "Timeline"),
      timeline,
      el("div", "khala-status-usage-group-title", "Usage"),
      usage,
      el("div", "khala-status-usage-group-title", "Runtime"),
      runtime,
      el("div", "khala-status-usage-group-title", "Provider Errors"),
      errors,
    )
  }

  const render = (): HTMLElement => {
    const section = el("section", "khala-settings-section khala-status-usage-section")
    sectionNode = section
    renderInto(section)
    return section
  }

  return { render, refresh }
}
