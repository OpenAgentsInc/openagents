import { Thread } from "@/components/assistant-ui/thread"
import { ThreadList } from "@/components/assistant-ui/thread-list"
import { Button } from "@/components/ui/button"
import { useCallback, useState } from "react"
import { createSession, sendPrompt, getSession } from "@/lib/tauri-acp"

export function AssistantSidebar() {
  const [testing, setTesting] = useState(false)
  const [lastStatus, setLastStatus] = useState<string>("")

  const handleTestACP = useCallback(async () => {
    try {
      setTesting(true)
      setLastStatus("Spawning agent…")

      // Use codex-exec path for now to translate codex exec --json events into ACP state
      const sessionId = await createSession("codex-exec")
      setLastStatus(`Session: ${sessionId}`)

      await sendPrompt(sessionId, "Hello from OpenAgents Tauri (Phase 1 test)")
      setLastStatus("Prompt sent. Fetching session…")

      const s = await getSession(sessionId)
      console.log("ACP getSession:", s)
      const last = s.messages?.[s.messages.length - 1]
      setLastStatus(last ? `Last message role=${last.role}` : "No messages yet")
    } catch (err) {
      console.error("ACP test error", err)
      setLastStatus(`Error: ${String(err)}`)
    } finally {
      setTesting(false)
    }
  }, [])

  return (
    <div className="flex h-full w-full">
      {/* Left Sidebar */}
      <div className="flex w-64 flex-col border-r border-zinc-800 bg-zinc-950">
        {/* Header */}
        <div className="select-none flex items-center gap-2 border-b border-zinc-800 p-4 pt-8">
          <div className="flex size-6 items-center justify-center rounded-[var(--radius-lg)]">
            <img src="/oalogo.png" alt="OpenAgents" className="size-6" />
          </div>
          <span className="font-semibold">OpenAgents</span>
        </div>

        {/* Thread List */}
        <div className="flex-1 overflow-auto p-2">
          <ThreadList />
        </div>

        {/* Footer / Manual Trigger */}
        <div className="border-t border-zinc-800 p-2">
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={handleTestACP}
              disabled={testing}
              title="Runs a simple create_session + send_prompt + get_session"
            >
              {testing ? "Testing…" : "Test ACP"}
            </Button>
            <div className="text-xs text-zinc-400 whitespace-pre-wrap break-words" title={lastStatus}>
              {lastStatus}
            </div>
          </div>
        </div>
      </div>

      {/* Main Chat Area */}
      <div className="flex-1">
        <Thread />
      </div>
    </div>
  );
}
