import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync } from "fs";
import { homedir } from "os";
import { join, resolve } from "path";

export interface SessionMessage {
  type: "message";
  timestamp: string;
  role: string;
  content: any;
}

export interface SessionHeader {
  type: "session";
  id: string;
  timestamp: string;
  cwd: string;
  provider?: string;
  model?: string;
}

export class SessionManager {
  private sessionFile: string | null = null;
  private enabled = true;

  constructor(private baseDir?: string) {}

  disable() {
    this.enabled = false;
  }

  private sessionDir(): string {
    const configDir = resolve(this.baseDir || process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi/agent/"));
    const cwdSafe = "--" + process.cwd().replace(/^[/\\]/, "").replace(/[/\\:]/g, "-") + "--";
    const dir = join(configDir, "sessions", cwdSafe);
    if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
    return dir;
  }

  start(provider?: string, model?: string) {
    if (!this.enabled) return;
    const dir = this.sessionDir();
    const id = Date.now().toString(36);
    this.sessionFile = join(dir, `${id}.jsonl`);
    const header: SessionHeader = {
      type: "session",
      id,
      timestamp: new Date().toISOString(),
      cwd: process.cwd(),
      provider,
      model,
    };
    appendFileSync(this.sessionFile, JSON.stringify(header) + "\n");
  }

  saveMessage(role: string, content: any) {
    if (!this.enabled || !this.sessionFile) return;
    const entry: SessionMessage = {
      type: "message",
      timestamp: new Date().toISOString(),
      role,
      content,
    };
    appendFileSync(this.sessionFile, JSON.stringify(entry) + "\n");
  }

  listSessions(): string[] {
    const dir = this.sessionDir();
    if (!existsSync(dir)) return [];
    return readdirSync(dir)
      .filter((f) => f.endsWith(".jsonl"))
      .sort((a, b) => statSync(join(dir, b)).mtime.getTime() - statSync(join(dir, a)).mtime.getTime())
      .map((f) => join(dir, f));
  }

  load(sessionPath: string): Array<SessionHeader | SessionMessage> {
    const path = resolve(sessionPath);
    if (!existsSync(path)) return [];
    return readFileSync(path, "utf8")
      .trim()
      .split("\n")
      .map((line) => {
        try {
          return JSON.parse(line);
        } catch {
          return null;
        }
      })
      .filter(Boolean) as Array<SessionHeader | SessionMessage>;
  }
}
