"use client";

import { AssistantRuntimeProvider, useLocalRuntime } from "@openagentsinc/assistant-ui-runtime";
import type { AttachmentAdapter } from "@openagentsinc/assistant-ui-runtime";
import { INTERNAL } from "@openagentsinc/assistant-ui-runtime";
import { useModelStore } from "@/lib/model-store";
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
  const ollamaAdapter = createOllamaAdapter({ baseURL: OLLAMA_BASE_URL, model: OLLAMA_MODEL });

  // Always call both hooks to maintain consistent hook order (Rules of Hooks)
  const ollamaRuntime = useLocalRuntime(ollamaAdapter, {
    adapters: {
      attachments: attachmentAdapter,
    },
  });
  const acpRuntime = useAcpRuntime();

  // Select which runtime to use based on model selection
  const runtime = selected === "ollama" ? ollamaRuntime : acpRuntime;

  return <AssistantRuntimeProvider runtime={runtime}>{children}</AssistantRuntimeProvider>;
}
