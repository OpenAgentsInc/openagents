import { Thread } from "@/components/assistant-ui/thread"
import { ThreadList } from "@/components/assistant-ui/thread-list"
import { Button } from "@/components/ui/button"
import { useCallback, useState } from "react"
import { createSession, sendPrompt, getSession, resolveAcpAgentPath } from "@/lib/tauri-acp"
import { useAcpStore } from "@/lib/acp-store"
import { useModelStore } from "@/lib/model-store"

export function AssistantSidebar() {
  const [testing, setTesting] = useState(false)
  const [lastStatus, setLastStatus] = useState<string>("")
  const startListening = useAcpStore((s) => s.startListening)
  const setActiveSession = useAcpStore((s) => s.setActiveSession)
  const liveText = useAcpStore((s) => s.liveText)
  const isStreaming = useAcpStore((s) => s.isStreaming)
  // Read selected model to decide behavior of Test ACP
  const model = useModelStore((s) => s.selected)

  const handleTestACP = useCallback(async () => {
    if (model === "ollama") {
      setLastStatus("Using Ollama runtime. Type in the composer to chat.")
      return
    }
    try {
      setTesting(true)
      setLastStatus("Checking ACP agent…")

      // Preflight: resolve codex-acp path for clearer errors
      try {
        const path = await resolveAcpAgentPath()
        setLastStatus(`ACP: ${path}\nSpawning agent…`)
      } catch (e) {
        setLastStatus(String(e))
        return
      }

      // Use ACP agent (codex-acp preferred; see backend resolution)
      const sessionId = await createSession("codex")
      setLastStatus(`Session: ${sessionId}`)

      // Begin streaming subscription for this session
      await startListening(sessionId)
      setActiveSession(sessionId)

      await sendPrompt(sessionId, "Hello from OpenAgents Tauri (Phase 1 test)")
      setLastStatus("Prompt sent. Fetching session…")

      const s = await getSession(sessionId)
      console.log("ACP getSession:", s)
      const last = s.messages?.[s.messages.length - 1]
      if (last && last.content?.[0]?.type === "text") {
        setLastStatus(`Assistant: ${(last.content[0] as any).text?.slice(0, 120) ?? ""}`)
      } else if (last) {
        setLastStatus(`Last message role=${last.role}`)
      } else {
        setLastStatus("No messages yet")
      }
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
            {/* Model picker moved to top app header */}
            <div className="text-xs text-zinc-400 whitespace-pre-wrap break-words" title={lastStatus}>
              {isStreaming ? "Live… " : ""}{lastStatus}
              {liveText ? `\n${liveText.slice(-200)}` : ""}
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
