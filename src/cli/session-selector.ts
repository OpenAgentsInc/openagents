import { basename } from "path";
import { SessionManager, type SessionMessage, type SessionHeader } from "./session-manager.js";

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

const summarizeLabel = (
  path: string,
  entries: Array<SessionHeader | SessionMessage>,
  maxLength = 80,
): string => {
  const firstMessage = entries.find((e): e is SessionMessage => e.type === "message");
  const content =
    typeof firstMessage?.content === "string" ? firstMessage.content : firstMessage?.content?.toString();
  const fallback = basename(path);
  if (!content || content.trim().length === 0) return fallback;
  const text = content.trim();
  return text.length > maxLength ? `${text.slice(0, maxLength - 3)}...` : text;
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
