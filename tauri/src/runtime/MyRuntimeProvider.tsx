"use client";

import { AssistantRuntimeProvider, useLocalRuntime } from "@assistant-ui/react";
import type { AttachmentAdapter } from "@assistant-ui/react";
import { INTERNAL } from "@assistant-ui/react";
import { useState } from "react";
import { useAcpSessionUpdates } from "@/lib/useAcpSessionUpdates";
import { useModelStore } from "@/lib/model-store";
import { createAcpAdapter } from "@/runtime/adapters/acp-adapter";
import { createOllamaAdapter } from "@/runtime/adapters/ollama-adapter";
import { OLLAMA_BASE_URL, OLLAMA_MODEL } from "@/config/ollama";
import { useAcpRuntime } from "@/runtime/useAcpRuntime";

const { generateId } = INTERNAL;

// Minimal attachment adapter; can be expanded later
const attachmentAdapter: AttachmentAdapter = {
  accept: "*/*",
  async add({ file }) {
    return {
      id: generateId(),
      file,
      type: "file",
      name: file.name,
      contentType: file.type,
      status: { type: "requires-action", reason: "composer-send" },
    };
  },
  async send(attachment) {
    return {
      ...attachment,
      content: [
        {
          type: "text",
          text: `[User attached a file: ${attachment.name}]`,
        },
      ],
      status: { type: "complete" },
    };
  },
  async remove() {
    // no-op
  },
};

export function MyRuntimeProvider({ children }: { children: React.ReactNode }) {
  const selected = useModelStore((s) => s.selected);
  const [activeSessionId, setActiveSessionId] = useState<string | undefined>();
  const session = useAcpSessionUpdates({ threadId: activeSessionId, debug: true });

  const acpAdapter = createAcpAdapter(session, { setActiveSessionId });
  const ollamaAdapter = createOllamaAdapter({ baseURL: OLLAMA_BASE_URL, model: OLLAMA_MODEL });
  const adapter = selected === "codex" ? acpAdapter : ollamaAdapter;

  // Feature flag: switch to ACP-native runtime (ExternalStore over WS) for the codex path
  const useAcpExternalStore = ((import.meta as any).env?.VITE_ACP_RUNTIME ?? "") === "1";

  const runtime = selected === "codex" && useAcpExternalStore
    ? useAcpRuntime()
    : useLocalRuntime(adapter, {
        adapters: {
          attachments: attachmentAdapter,
        },
      });

  return <AssistantRuntimeProvider runtime={runtime}>{children}</AssistantRuntimeProvider>;
}
