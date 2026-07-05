import type {
  KhalaCodeDesktopUpdaterActionResult,
  KhalaCodeDesktopUpdaterStatus,
} from "../shared/rpc"

/**
 * "Updates" settings row for the #8440 in-app updater plumbing. Mounted
 * alongside `mountClaudeSettingsSection` into the shared settings container
 * (see `src/ui/main.ts`).
 */

export type KhalaCodeUpdaterSettingsSectionHandle = {
  readonly applyStatus: (status: KhalaCodeDesktopUpdaterStatus) => void
  readonly refresh: () => Promise<void>
}

export type KhalaCodeUpdaterSettingsSectionOptions = {
  readonly check: () => Promise<KhalaCodeDesktopUpdaterActionResult>
  readonly download: () => Promise<KhalaCodeDesktopUpdaterActionResult>
  readonly install: () => Promise<KhalaCodeDesktopUpdaterActionResult>
  readonly openReleaseNotes: (url: string) => unknown
  readonly status: () => Promise<KhalaCodeDesktopUpdaterStatus>
}

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

const metric = (label: string, value: string): HTMLElement => {
  const item = el("div", "khala-settings-metric")
  item.append(
    el("span", "khala-settings-metric-label", label),
    el("span", "khala-settings-metric-value", value),
  )
  return item
}

type UpdaterActionKind = "check" | "download" | "install" | "none"

const describeState = (
  status: KhalaCodeDesktopUpdaterStatus,
): { readonly action: UpdaterActionKind; readonly actionLabel: string; readonly description: string } => {
  const state = status.state
  switch (state.status) {
    case "idle":
      return { action: "check", actionLabel: "Check for Updates", description: "Not checked yet" }
    case "checking":
      return { action: "none", actionLabel: "Checking…", description: "Checking for updates" }
    case "up_to_date":
      return {
        action: "check",
        actionLabel: "Check for Updates",
        description: `Up to date (${state.version})`,
      }
    case "available":
      return {
        action: "download",
        actionLabel: "Download Update",
        description: `Update available: ${state.version}`,
      }
    case "downloading":
      return {
        action: "none",
        actionLabel: "Downloading…",
        description: state.progressPercent === null
          ? `Downloading ${state.version}`
          : `Downloading ${state.version} (${Math.round(state.progressPercent)}%)`,
      }
    case "ready":
      // Downloaded and install-ready — installing is always a separate,
      // explicit click; it never happens automatically.
      return {
        action: "install",
        actionLabel: "Restart to Install",
        description: `Update ${state.version} downloaded and ready to install`,
      }
    case "installing":
      return { action: "none", actionLabel: "Installing…", description: `Installing ${state.version}` }
    case "error":
      return {
        action: state.retryable ? "check" : "none",
        actionLabel: state.retryable ? "Retry" : "Update Error",
        description: state.message,
      }
  }
}

export const mountKhalaCodeUpdaterSettingsSection = (
  container: HTMLElement,
  options: KhalaCodeUpdaterSettingsSectionOptions,
): KhalaCodeUpdaterSettingsSectionHandle => {
  let status: KhalaCodeDesktopUpdaterStatus | null = null
  let busy = false

  const runAction = async (kind: UpdaterActionKind): Promise<void> => {
    if (busy) return
    busy = true
    render()
    try {
      const result = kind === "check"
        ? await options.check()
        : kind === "download"
          ? await options.download()
          : kind === "install"
            ? await options.install()
            : null
      if (result !== null) status = result.status
    } finally {
      busy = false
      render()
    }
  }

  const render = (): void => {
    const existing = container.querySelector(".khala-settings-section--updater")
    existing?.remove()
    const section = el("section", "khala-settings-section khala-settings-section--updater")
    section.append(el("h3", "khala-settings-section-title", "Updates"))

    if (status === null) {
      section.append(el("div", "khala-settings-empty", "Updater status has not loaded yet."))
      container.append(section)
      return
    }

    if (!status.enabled) {
      section.append(
        el("div", "khala-settings-empty", "In-app updates are disabled for this build."),
      )
      container.append(section)
      return
    }

    const described = describeState(status)
    section.append(
      metric("Channel", status.channel),
      metric("Current version", status.currentVersion),
      metric("Status", described.description),
    )

    const actions = el("div", "khala-settings-updater-actions")
    const actionButton = el("button", "khala-settings-refresh", described.actionLabel)
    actionButton.type = "button"
    actionButton.disabled = busy || described.action === "none"
    actionButton.addEventListener("click", () => void runAction(described.action))
    actions.append(actionButton)

    if (status.releaseNotesUrl.length > 0) {
      const releaseNotes = el("button", "khala-settings-updater-release-notes", "Release Notes")
      releaseNotes.type = "button"
      releaseNotes.addEventListener("click", () => options.openReleaseNotes(status!.releaseNotesUrl))
      actions.append(releaseNotes)
    }
    section.append(actions)
    container.append(section)
  }

  return {
    applyStatus(next) {
      status = next
      render()
    },
    async refresh() {
      status = await options.status()
      render()
    },
  }
}
