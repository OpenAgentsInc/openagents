import { invoke } from "@tauri-apps/api/core";

export async function createSession(agentType?: "claude-code" | "codex" | "codex-exec", cwd?: string): Promise<string> {
  return invoke("create_session", { agentType, cwd });
}

export async function sendPrompt(sessionId: string, text: string): Promise<void> {
  return invoke("send_prompt", { sessionId, text });
}

export async function getSession(sessionId: string): Promise<any> {
  return invoke("get_session", { sessionId });
}

export async function resolveAcpAgentPath(): Promise<string> {
  return invoke("resolve_acp_agent_path");
}

export async function validateDirectory(path: string): Promise<boolean> {
  return invoke("validate_directory", { path });
}

export async function pickDirectory(): Promise<string | null> {
  return invoke("pick_directory");
}
