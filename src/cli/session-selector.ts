import { basename } from "path";
import { SessionManager, type SessionMessageEntry, type SessionHeader } from "./session-manager.js";

export interface SessionSummary {
  path: string;
  label: string;
}

export interface SessionSelectorOptions {
  baseDir?: string;
  cwd?: string;
}

const withWorkingDir = <A>(cwd: string | undefined, fn: () => A): A => {
  if (!cwd) return fn();
  const prev = process.cwd();
  process.chdir(cwd);
  try {
    return fn();
  } finally {
    process.chdir(prev);
  }
};

const summarizeLabel = (path: string, entries: Array<SessionHeader | SessionMessageEntry>, maxLength = 80): string => {
  const fallback = basename(path);
  const firstMessage = entries.find((e): e is SessionMessageEntry => e.type === "message");
  if (!firstMessage) return fallback;

  const message = (firstMessage as any).message ?? (firstMessage as any).content;
  const text =
    typeof message === "string"
      ? message
      : typeof message?.content === "string"
        ? message.content
        : Array.isArray(message?.content)
          ? message.content
              .filter((c: any) => c?.type === "text")
              .map((c: any) => c.text)
              .join(" ")
          : "";

  const trimmed = text.trim();
  if (!trimmed) return fallback;
  return trimmed.length > maxLength ? `${trimmed.slice(0, maxLength - 3)}...` : trimmed;
};

export const getSessionSummaries = (options: SessionSelectorOptions = {}): SessionSummary[] =>
  withWorkingDir(options.cwd, () => {
    const manager = new SessionManager(options.baseDir);
    const sessions = manager.listSessions();
    return sessions.map((path) => ({
      path,
      label: summarizeLabel(path, manager.load(path)),
    }));
  });

export const findLatestSession = (options: SessionSelectorOptions = {}): string | null =>
  withWorkingDir(options.cwd, () => {
    const manager = new SessionManager(options.baseDir);
    const sessions = manager.listSessions();
    return sessions[0] ?? null;
  });
