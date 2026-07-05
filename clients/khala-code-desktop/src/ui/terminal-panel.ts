import type {
  KhalaCodeTerminalWorkbenchProjection,
} from "../shared/terminal-workbench"

export type KhalaCodeTerminalPanelHandle = Readonly<{
  refresh: () => Promise<void>
  setVisible: (visible: boolean) => void
}>

export type KhalaCodeTerminalPanelOptions = Readonly<{
  clean: () => Promise<void>
  copy: (text: string) => Promise<void>
  fetch: () => Promise<KhalaCodeTerminalWorkbenchProjection>
  terminate: (processId: string) => Promise<void>
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

export const mountKhalaCodeTerminalPanel = (
  container: HTMLElement,
  options: KhalaCodeTerminalPanelOptions,
): KhalaCodeTerminalPanelHandle => {
  let projection: KhalaCodeTerminalWorkbenchProjection | null = null
  let status = ""
  let visible = !container.hidden

  const refresh = async (): Promise<void> => {
    try {
      projection = await options.fetch()
      status = ""
    } catch (error) {
      status = error instanceof Error ? error.message : String(error)
    }
    render()
  }

  const run = async (task: () => Promise<void>, message: string): Promise<void> => {
    try {
      await task()
      status = message
      try {
        projection = await options.fetch()
      } catch {
        // Keep the completed action visible when follow-up refresh fails.
      }
      render()
    } catch (error) {
      status = error instanceof Error ? error.message : String(error)
      render()
    }
  }

  const render = (): void => {
    container.replaceChildren()
    container.hidden = !visible
    const shell = el("div", "khala-terminal-panel-shell")
    const header = el("div", "khala-terminal-panel-header")
    header.append(
      el("h2", "khala-terminal-panel-title", "Terminal"),
      el("p", "khala-terminal-panel-boundary", projection?.boundary === "active_thread"
        ? `Bound to active session ${projection.activeThreadId}`
        : "No active session; terminal transport is disabled."),
    )

    const actions = el("div", "khala-terminal-panel-actions")
    const newTerminal = el("button", "khala-terminal-panel-action", "New Terminal")
    newTerminal.type = "button"
    newTerminal.disabled = true
    newTerminal.title = "Interactive local PTY creation waits for the Khala-owned terminal bridge."
    const refreshButton = el("button", "khala-terminal-panel-action", "Refresh")
    refreshButton.type = "button"
    refreshButton.addEventListener("click", () => {
      void refresh()
    })
    const cleanButton = el("button", "khala-terminal-panel-action", "Clean Exited")
    cleanButton.type = "button"
    cleanButton.disabled = projection?.activeThreadId === null
    cleanButton.addEventListener("click", () => {
      void run(options.clean, "Exited terminals cleaned.")
    })
    actions.append(newTerminal, refreshButton, cleanButton)
    header.append(actions)
    shell.append(header)

    if (status.length > 0) shell.append(el("div", "khala-terminal-panel-status", status))

    if (projection === null) {
      shell.append(el("div", "khala-terminal-panel-empty", "Terminal state has not loaded yet."))
      container.append(shell)
      return
    }
    if (projection.activeThreadId === null) {
      shell.append(el("div", "khala-terminal-panel-empty", "Open a session to inspect its background terminals."))
      container.append(shell)
      return
    }
    if (projection.tabs.length === 0) {
      shell.append(el("div", "khala-terminal-panel-empty", "No background terminals for this session."))
      container.append(shell)
      return
    }

    const tabs = el("div", "khala-terminal-tabs")
    const activeTab = projection.tabs.find(tab => tab.processId === projection?.activeProcessId) ?? projection.tabs[0]
    for (const tab of projection.tabs) {
      const button = el("button", "khala-terminal-tab", tab.title)
      button.type = "button"
      button.dataset.processId = tab.processId
      button.dataset.active = String(tab.processId === activeTab?.processId)
      button.dataset.status = tab.status
      button.addEventListener("click", () => {
        projection = {
          ...projection!,
          activeProcessId: tab.processId,
        }
        render()
      })
      tabs.append(button)
    }
    shell.append(tabs)

    if (activeTab !== undefined) {
      const body = el("section", "khala-terminal-body")
      body.dataset.processId = activeTab.processId
      body.append(
        el("div", "khala-terminal-meta", `${activeTab.status} / ${activeTab.cwd ?? "workspace unknown"}`),
        el("pre", "khala-terminal-output", activeTab.outputPreview || "No captured output."),
      )
      const bodyActions = el("div", "khala-terminal-panel-actions")
      const copy = el("button", "khala-terminal-panel-action", "Copy Output")
      copy.type = "button"
      copy.addEventListener("click", () => {
        void run(() => options.copy(activeTab.outputPreview), "Terminal output copied.")
      })
      const terminate = el("button", "khala-terminal-panel-action", "Terminate")
      terminate.type = "button"
      terminate.disabled = activeTab.status !== "running"
      terminate.addEventListener("click", () => {
        void run(() => options.terminate(activeTab.processId), "Terminal terminated.")
      })
      bodyActions.append(copy, terminate)
      body.append(bodyActions)
      shell.append(body)
    }

    container.append(shell)
  }

  render()

  return {
    refresh,
    setVisible(next) {
      visible = next
      if (visible) void refresh()
      render()
    },
  }
}
