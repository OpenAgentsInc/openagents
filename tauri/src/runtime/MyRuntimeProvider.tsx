"use client";

import { AssistantRuntimeProvider } from "@openagentsinc/assistant-ui-runtime";
import { useAcpRuntime } from "@/runtime/useAcpRuntime";

export function MyRuntimeProvider({ children }: { children: React.ReactNode }) {
  // Use ACP runtime exclusively (supports Claude Code and Codex agents)
  const runtime = useAcpRuntime();

  return <AssistantRuntimeProvider runtime={runtime}>{children}</AssistantRuntimeProvider>;
}
