import { randomUUID } from "crypto";
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync } from "fs";
import { homedir } from "os";
import { join, resolve } from "path";
import {
  compactMessages,
  checkBackpressure,
  estimateTokensForMessages,
  getContextSummary,
  needsCompaction,
  type CompactionConfig,
  type CompactionResult,
  type BackpressureStatus,
  DEFAULT_COMPACTION_CONFIG,
} from "../agent/context-compaction.js";

export type SessionEntry =
  | SessionHeader
  | SessionMessageEntry
  | ThinkingLevelChangeEntry
  | ModelChangeEntry;

export interface SessionHeader {
  type: "session";
  id: string;
  timestamp: string;
  cwd: string;
  provider?: string;
  model?: string;
  thinkingLevel?: string;
}

export interface SessionMessageEntry {
  type: "message";
  timestamp: string;
  message: any;
}

export interface ThinkingLevelChangeEntry {
  type: "thinking_level_change";
  timestamp: string;
  thinkingLevel: string;
}

export interface ModelChangeEntry {
  type: "model_change";
  timestamp: string;
  provider: string;
  model: string;
}

export interface SessionManagerOptions {
  baseDir?: string;
  continueSession?: boolean;
  sessionPath?: string;
}

export class SessionManager {
  private sessionId!: string;
  private sessionFile!: string;
  private sessionDir: string;
  private enabled = true;
  private sessionInitialized = false;
  private pendingMessages: SessionEntry[] = [];

  constructor(options?: SessionManagerOptions | string) {
    const opts: SessionManagerOptions = typeof options === "string" ? { baseDir: options } : options ?? {};
    this.sessionDir = this.getSessionDirectory(opts.baseDir);

    if (opts.sessionPath) {
      this.sessionFile = resolve(opts.sessionPath);
      this.loadSessionId();
      this.sessionInitialized = existsSync(this.sessionFile);
    } else if (opts.continueSession) {
      const mostRecent = this.findMostRecentlyModifiedSession();
      if (mostRecent) {
        this.sessionFile = mostRecent;
        this.loadSessionId();
        this.sessionInitialized = true;
      } else {
        this.initNewSession();
      }
    } else {
      this.initNewSession();
    }
  }

  disable() {
    this.enabled = false;
  }

  private getSessionDirectory(baseDir?: string): string {
    const configDir = resolve(baseDir || process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi/agent/"));
    const cwdSafe = "--" + process.cwd().replace(/^[/\\]/, "").replace(/[/\\:]/g, "-") + "--";
    const dir = join(configDir, "sessions", cwdSafe);
    if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
    return dir;
  }

  private initNewSession(): void {
    this.sessionId = randomUUID();
    const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
    this.sessionFile = join(this.sessionDir, `${timestamp}_${this.sessionId}.jsonl`);
  }

  private findMostRecentlyModifiedSession(): string | null {
    try {
      const files = readdirSync(this.sessionDir)
        .filter((f) => f.endsWith(".jsonl"))
        .map((f) => ({
          name: f,
          path: join(this.sessionDir, f),
          mtime: statSync(join(this.sessionDir, f)).mtime,
        }))
        .sort((a, b) => b.mtime.getTime() - a.mtime.getTime());

      return files[0]?.path || null;
    } catch {
      return null;
    }
  }

  private loadSessionId(): void {
    if (!existsSync(this.sessionFile)) return;

    const lines = readFileSync(this.sessionFile, "utf8").trim().split("\n");
    for (const line of lines) {
      try {
        const entry = JSON.parse(line);
        if (entry.type === "session") {
          this.sessionId = entry.id;
          return;
        }
      } catch {
        // ignore malformed lines
      }
    }
    this.sessionId = randomUUID();
  }

  start(provider?: string, model?: string, thinkingLevel: string = "off") {
    if (!this.enabled || this.sessionInitialized) return;
    this.sessionInitialized = true;

    const header: SessionHeader = {
      type: "session",
      id: this.sessionId,
      timestamp: new Date().toISOString(),
      cwd: process.cwd(),
      thinkingLevel,
    };
    if (provider) header.provider = provider;
    if (model) header.model = model;
    appendFileSync(this.sessionFile, JSON.stringify(header) + "\n");

    for (const entry of this.pendingMessages) {
      appendFileSync(this.sessionFile, JSON.stringify(entry) + "\n");
    }
    this.pendingMessages = [];
  }

  reset(): void {
    this.pendingMessages = [];
    this.sessionInitialized = false;
    this.initNewSession();
  }

  saveMessage(message: any) {
    if (!this.enabled) return;
    const entry: SessionMessageEntry = {
      type: "message",
      timestamp: new Date().toISOString(),
      message,
    };

    if (!this.sessionInitialized) {
      this.pendingMessages.push(entry);
    } else {
      appendFileSync(this.sessionFile, JSON.stringify(entry) + "\n");
    }
  }

  saveThinkingLevelChange(thinkingLevel: string): void {
    if (!this.enabled) return;
    const entry: ThinkingLevelChangeEntry = {
      type: "thinking_level_change",
      timestamp: new Date().toISOString(),
      thinkingLevel,
    };

    if (!this.sessionInitialized) {
      this.pendingMessages.push(entry);
    } else {
      appendFileSync(this.sessionFile, JSON.stringify(entry) + "\n");
    }
  }

  saveModelChange(provider: string, model: string): void {
    if (!this.enabled) return;
    const entry: ModelChangeEntry = {
      type: "model_change",
      timestamp: new Date().toISOString(),
      provider,
      model,
    };

    if (!this.sessionInitialized) {
      this.pendingMessages.push(entry);
    } else {
      appendFileSync(this.sessionFile, JSON.stringify(entry) + "\n");
    }
  }

  listSessions(): string[] {
    if (!existsSync(this.sessionDir)) return [];
    return readdirSync(this.sessionDir)
      .filter((f) => f.endsWith(".jsonl"))
      .sort((a, b) => statSync(join(this.sessionDir, b)).mtime.getTime() - statSync(join(this.sessionDir, a)).mtime.getTime())
      .map((f) => join(this.sessionDir, f));
  }

  load(sessionPath: string): SessionEntry[] {
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
      .filter(Boolean) as SessionEntry[];
  }

  loadMessages(): any[] {
    if (!existsSync(this.sessionFile)) return [];
    const messages: any[] = [];
    const lines = readFileSync(this.sessionFile, "utf8").trim().split("\n");

    for (const line of lines) {
      try {
        const entry = JSON.parse(line);
        if (entry.type === "message") {
          messages.push(entry.message);
        }
      } catch {
        // ignore malformed lines
      }
    }
    return messages;
  }

  loadThinkingLevel(): string {
    if (!existsSync(this.sessionFile)) return "off";

    const lines = readFileSync(this.sessionFile, "utf8").trim().split("\n");
    let lastThinking = "off";
    for (const line of lines) {
      try {
        const entry = JSON.parse(line);
        if (entry.type === "session" && entry.thinkingLevel) {
          lastThinking = entry.thinkingLevel;
        } else if (entry.type === "thinking_level_change" && entry.thinkingLevel) {
          lastThinking = entry.thinkingLevel;
        }
      } catch {
        // ignore
      }
    }
    return lastThinking;
  }

  loadModel(): { provider: string; model: string } | null {
    if (!existsSync(this.sessionFile)) return null;

    const lines = readFileSync(this.sessionFile, "utf8").trim().split("\n");
    let lastProvider: string | null = null;
    let lastModel: string | null = null;

    for (const line of lines) {
      try {
        const entry = JSON.parse(line);
        if (entry.type === "session" && entry.provider && entry.model) {
          lastProvider = entry.provider;
          lastModel = entry.model;
        } else if (entry.type === "model_change" && entry.provider && entry.model) {
          lastProvider = entry.provider;
          lastModel = entry.model;
        }
      } catch {
        // ignore
      }
    }

    if (lastProvider && lastModel) return { provider: lastProvider, model: lastModel };
    return null;
  }

  getSessionId(): string {
    return this.sessionId;
  }

  getSessionFile(): string {
    return this.sessionFile;
  }

  setSessionFile(path: string): void {
    this.sessionFile = resolve(path);
    this.loadSessionId();
    this.sessionInitialized = existsSync(path);
  }

  loadAllSessions(): Array<{
    path: string;
    id: string;
    created: Date;
    modified: Date;
    messageCount: number;
    firstMessage: string;
    allMessagesText: string;
  }> {
    const sessions: Array<{
      path: string;
      id: string;
      created: Date;
      modified: Date;
      messageCount: number;
      firstMessage: string;
      allMessagesText: string;
    }> = [];

    try {
      const files = readdirSync(this.sessionDir)
        .filter((f) => f.endsWith(".jsonl"))
        .map((f) => join(this.sessionDir, f));

      for (const file of files) {
        try {
          const stats = statSync(file);
          const content = readFileSync(file, "utf8");
          const lines = content.trim().split("\n");

          let sessionId = "";
          let created = stats.birthtime;
          let messageCount = 0;
          let firstMessage = "";
          const allMessages: string[] = [];

          for (const line of lines) {
            try {
              const entry = JSON.parse(line);
              if (entry.type === "session" && !sessionId) {
                sessionId = entry.id;
                created = new Date(entry.timestamp);
              }
              if (entry.type === "message") {
                messageCount++;
                const textContent = this.extractText(entry.message);
                if (textContent) {
                  allMessages.push(textContent);
                  if (!firstMessage) {
                    firstMessage = textContent;
                  }
                }
              }
            } catch {
              // ignore malformed lines
            }
          }

          sessions.push({
            path: file,
            id: sessionId || "unknown",
            created,
            modified: stats.mtime,
            messageCount,
            firstMessage: firstMessage || "(no messages)",
            allMessagesText: allMessages.join(" "),
          });
        } catch (error) {
          console.error(`Failed to read session file ${file}:`, error);
        }
      }

      sessions.sort((a, b) => b.modified.getTime() - a.modified.getTime());
    } catch (error) {
      console.error("Failed to load sessions:", error);
    }

    return sessions;
  }

  shouldInitializeSession(messages: any[]): boolean {
    if (this.sessionInitialized) return false;
    const userMessages = messages.filter((m) => m.role === "user");
    const assistantMessages = messages.filter((m) => m.role === "assistant");
    return userMessages.length >= 1 && assistantMessages.length >= 1;
  }

  createBranchedSession(state: { model: { provider: string; id: string }; thinkingLevel: string; messages: any[] }, branchFromIndex: number): string {
    const newSessionId = randomUUID();
    const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
    const newSessionFile = join(this.sessionDir, `${timestamp}_${newSessionId}.jsonl`);

    const entry: SessionHeader = {
      type: "session",
      id: newSessionId,
      timestamp: new Date().toISOString(),
      cwd: process.cwd(),
      provider: state.model.provider,
      model: state.model.id,
      thinkingLevel: state.thinkingLevel,
    };
    appendFileSync(newSessionFile, JSON.stringify(entry) + "\n");

    if (branchFromIndex >= 0) {
      const messagesToWrite = state.messages.slice(0, branchFromIndex + 1);
      for (const message of messagesToWrite) {
        const messageEntry: SessionMessageEntry = {
          type: "message",
          timestamp: new Date().toISOString(),
          message,
        };
        appendFileSync(newSessionFile, JSON.stringify(messageEntry) + "\n");
      }
    }

    return newSessionFile;
  }

  private extractText(message: any): string {
    if (!message) return "";
    if (typeof message === "string") return message;
    if (typeof message.content === "string") return message.content;
    if (Array.isArray(message.content)) {
      return message.content
        .filter((c: any) => c?.type === "text")
        .map((c: any) => c.text)
        .join(" ");
    }
    return "";
  }

  // ===========================================================================
  // Context Compaction & Backpressure
  // ===========================================================================

  /**
   * Check if messages need compaction based on current backpressure.
   *
   * @param messages - The messages array to check
   * @param config - Optional compaction configuration
   * @returns True if compaction is needed
   */
  checkNeedsCompaction(messages: any[], config?: Partial<CompactionConfig>): boolean {
    return needsCompaction(messages, config);
  }

  /**
   * Get backpressure status for the given messages.
   *
   * @param messages - The messages array to check
   * @param config - Optional compaction configuration
   * @returns BackpressureStatus with current usage info
   */
  getBackpressureStatus(messages: any[], config?: Partial<CompactionConfig>): BackpressureStatus {
    const fullConfig: CompactionConfig = { ...DEFAULT_COMPACTION_CONFIG, ...config };
    return checkBackpressure(messages, fullConfig);
  }

  /**
   * Compact messages using the configured strategy.
   *
   * Supported strategies:
   * - "turns": Keep last N conversation turns (pi-mono style, default 50)
   * - "tokens": Keep messages within a token budget
   * - "adaptive": Combine both strategies (recommended)
   *
   * @param messages - The messages array to compact
   * @param config - Compaction configuration
   * @returns CompactionResult with compacted messages and stats
   *
   * @example
   * ```ts
   * // Turn-based compaction (pi-mono style)
   * const result = sessionManager.compactMessages(messages, {
   *   strategy: "turns",
   *   maxTurns: 50,
   * });
   *
   * // Token-based with model context window
   * const result = sessionManager.compactMessages(messages, {
   *   strategy: "tokens",
   *   maxTokens: model.contextWindow * 0.7,
   * });
   *
   * // Adaptive (recommended for long sessions)
   * const result = sessionManager.compactMessages(messages, {
   *   strategy: "adaptive",
   *   maxTurns: 50,
   *   maxTokens: 100000,
   * });
   * ```
   */
  compactMessages(messages: any[], config?: Partial<CompactionConfig>): CompactionResult {
    return compactMessages(messages, config);
  }

  /**
   * Load messages with automatic compaction if needed.
   *
   * This is a convenience method that loads messages and applies compaction
   * if backpressure threshold is exceeded.
   *
   * @param config - Optional compaction configuration
   * @returns Object with messages (possibly compacted) and metadata
   */
  loadMessagesWithCompaction(config?: Partial<CompactionConfig>): {
    messages: any[];
    wasCompacted: boolean;
    compactionResult?: CompactionResult;
  } {
    const messages = this.loadMessages();

    if (messages.length === 0) {
      return { messages, wasCompacted: false };
    }

    const fullConfig = { ...DEFAULT_COMPACTION_CONFIG, ...config };
    const status = this.getBackpressureStatus(messages, fullConfig);

    if (status.action !== "compact") {
      return { messages, wasCompacted: false };
    }

    const result = this.compactMessages(messages, fullConfig);
    return {
      messages: result.messages,
      wasCompacted: result.wasCompacted,
      compactionResult: result,
    };
  }

  /**
   * Estimate tokens for the given messages.
   *
   * @param messages - The messages array
   * @returns Estimated token count
   */
  estimateTokens(messages: any[]): number {
    return estimateTokensForMessages(messages);
  }

  /**
   * Get a summary string of context usage for logging/debugging.
   *
   * @param messages - The messages array
   * @param config - Optional compaction configuration
   * @returns Human-readable summary string
   */
  getContextSummary(messages: any[], config?: Partial<CompactionConfig>): string {
    return getContextSummary(messages, config);
  }
}
