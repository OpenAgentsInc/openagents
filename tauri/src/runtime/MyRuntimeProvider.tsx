"use client";

import { AssistantRuntimeProvider } from "@openagentsinc/assistant-ui-runtime";
import { useModelStore } from "@/lib/model-store";
import { useAcpRuntime } from "@/runtime/useAcpRuntime";
import { useOllamaRuntime } from "@/runtime/useOllamaRuntime";

export function MyRuntimeProvider({ children }: { children: React.ReactNode }) {
  const selected = useModelStore((s) => s.selected);

  // Always call both hooks to maintain consistent hook order (Rules of Hooks)
  const ollamaRuntime = useOllamaRuntime();
  const acpRuntime = useAcpRuntime();

  // Select which runtime to use based on model selection
  const runtime = selected === "ollama" ? ollamaRuntime : acpRuntime;

  return <AssistantRuntimeProvider runtime={runtime}>{children}</AssistantRuntimeProvider>;
}
