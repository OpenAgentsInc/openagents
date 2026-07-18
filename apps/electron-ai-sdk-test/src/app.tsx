import { useChat } from "@ai-sdk/react";
import { DefaultChatTransport, type UIMessage } from "ai";
import { ArrowUp, Square } from "lucide-react";
import { FormEvent, KeyboardEvent, useEffect, useMemo, useState } from "react";

export function App() {
  const [endpoint, setEndpoint] = useState<string>();
  const [startupError, setStartupError] = useState<string>();

  useEffect(() => {
    void window.harnessDesktop
      .getEndpoint()
      .then(setEndpoint)
      .catch(() => setStartupError("The local harness service did not start."));
  }, []);

  if (startupError !== undefined) {
    return <StartupState message={startupError} />;
  }

  if (endpoint === undefined) {
    return <StartupState message="Starting the local harness…" />;
  }

  return <HarnessChat endpoint={endpoint} />;
}

function HarnessChat({ endpoint }: { endpoint: string }) {
  const [input, setInput] = useState("");
  const chatId = useMemo(() => crypto.randomUUID(), []);
  const transport = useMemo(
    () => new DefaultChatTransport({ api: `${endpoint}/api/chat` }),
    [endpoint],
  );
  const { messages, sendMessage, status, stop, error } = useChat({
    id: chatId,
    transport,
  });
  const isBusy = status === "submitted" || status === "streaming";

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    const text = input.trim();
    if (!text || isBusy) return;
    void sendMessage({ text });
    setInput("");
  }

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): void {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      event.currentTarget.form?.requestSubmit();
    }
  }

  return (
    <main className="flex h-screen min-h-0 flex-col bg-[var(--background)] text-[var(--foreground)]">
      <header className="app-drag flex h-14 shrink-0 items-center border-b border-white/8 px-5">
        <div className="app-no-drag flex items-center gap-3">
          <span className="flex h-6 w-6 items-center justify-center rounded-md bg-[var(--primary)] text-xs font-semibold text-slate-950">
            A
          </span>
          <div>
            <h1 className="text-sm font-medium tracking-[-0.01em]">AI SDK Harness</h1>
            <p className="text-[11px] text-[var(--muted-foreground)]">Electron test</p>
          </div>
        </div>
        <div className="app-no-drag ml-auto flex items-center gap-2 text-xs text-[var(--muted-foreground)]">
          <span className="h-1.5 w-1.5 rounded-full bg-sky-400" />
          Local Codex
        </div>
      </header>

      <section className="mx-auto flex min-h-0 w-full max-w-3xl flex-1 flex-col px-5">
        <div className="min-h-0 flex-1 overflow-y-auto py-8" aria-live="polite">
          {messages.length === 0 ? <EmptyState /> : <MessageList messages={messages} />}
          {isBusy ? <p className="mt-5 text-xs text-[var(--muted-foreground)]">Codex is working…</p> : null}
          {error !== undefined ? (
            <p className="mt-5 rounded-md border border-red-400/20 bg-red-400/8 px-3 py-2 text-sm text-red-200">
              {error.message || "The local harness could not complete that turn."}
            </p>
          ) : null}
        </div>

        <form className="app-no-drag shrink-0 pb-5" onSubmit={submit}>
          <div className="rounded-xl border border-white/10 bg-white/[0.045] p-2 shadow-[0_12px_35px_rgba(0,0,0,0.18)] focus-within:border-sky-400/55 focus-within:ring-2 focus-within:ring-sky-400/15">
            <textarea
              aria-label="Message the local Codex harness"
              className="min-h-20 w-full resize-none bg-transparent px-2 py-1 text-sm leading-6 text-[var(--foreground)] outline-none placeholder:text-[var(--muted-foreground)]"
              disabled={isBusy}
              onChange={(event) => setInput(event.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Ask Codex to inspect or change the local harness workspace…"
              value={input}
            />
            <div className="flex items-center justify-between px-1 pt-1">
              <span className="text-[11px] text-[var(--muted-foreground)]">⌘ ↵ to send</span>
              {isBusy ? (
                <button
                  aria-label="Stop generation"
                  className="inline-flex h-8 items-center gap-1.5 rounded-md bg-white/10 px-2.5 text-xs font-medium text-white transition hover:bg-white/15"
                  onClick={() => stop()}
                  type="button"
                >
                  <Square size={12} fill="currentColor" /> Stop
                </button>
              ) : (
                <button
                  aria-label="Send message"
                  className="inline-flex h-8 w-8 items-center justify-center rounded-md bg-[var(--primary)] text-slate-950 transition hover:bg-sky-300 disabled:cursor-not-allowed disabled:opacity-40"
                  disabled={!input.trim()}
                  type="submit"
                >
                  <ArrowUp size={16} strokeWidth={2.5} />
                </button>
              )}
            </div>
          </div>
          <p className="mt-2 text-center text-[11px] text-[var(--muted-foreground)]">
            Experimental owner-local harness. This is not a production containment boundary.
          </p>
        </form>
      </section>
    </main>
  );
}

function EmptyState() {
  return (
    <div className="flex h-full min-h-72 flex-col items-center justify-center text-center">
      <span className="mb-4 flex h-10 w-10 items-center justify-center rounded-xl border border-sky-300/20 bg-sky-400/10 text-base text-sky-300">
        A
      </span>
      <h2 className="text-base font-medium">Start a local Codex session</h2>
      <p className="mt-2 max-w-sm text-sm leading-6 text-[var(--muted-foreground)]">
        Messages stream through AI SDK UI into a native Codex harness session.
      </p>
    </div>
  );
}

function MessageList({ messages }: { messages: UIMessage[] }) {
  return (
    <div className="space-y-7">
      {messages.map((message) => (
        <article className="flex gap-3" key={message.id}>
          <span
            className={`mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-[10px] font-semibold ${
              message.role === "user"
                ? "bg-white/10 text-white"
                : "bg-sky-400/15 text-sky-300"
            }`}
          >
            {message.role === "user" ? "YOU" : "AI"}
          </span>
          <div className="min-w-0 flex-1 pt-0.5 text-sm leading-6">
            {message.parts.map((part, index) => {
              if (part.type === "text") {
                return (
                  <p className="whitespace-pre-wrap" key={`${message.id}-${index}`}>
                    {part.text}
                  </p>
                );
              }
              if (part.type === "reasoning") {
                return (
                  <p className="mt-2 text-xs italic text-[var(--muted-foreground)]" key={`${message.id}-${index}`}>
                    {part.text}
                  </p>
                );
              }
              if (part.type.startsWith("tool-") || part.type === "dynamic-tool") {
                return (
                  <ToolEventInspector key={`${message.id}-${index}`} part={part} />
                );
              }
              return null;
            })}
          </div>
        </article>
      ))}
    </div>
  );
}

function ToolEventInspector({ part }: { part: UIMessage["parts"][number] }) {
  const event = part as unknown as Record<string, unknown>;
  const toolName =
    typeof event.toolName === "string"
      ? event.toolName
      : part.type === "dynamic-tool"
        ? "harness event"
        : part.type.slice("tool-".length);
  const state = typeof event.state === "string" ? event.state : "received";
  const details = [
    ["Input", event.input],
    ["Output", event.output],
    ["Error", event.errorText ?? event.error],
  ] as const;

  return (
    <details className="group mt-3 overflow-hidden rounded-lg border border-sky-300/15 bg-sky-400/[0.045]" open={state === "input-streaming"}>
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-xs marker:content-none">
        <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded bg-sky-400/15 font-mono text-[10px] text-sky-300">
          ›_
        </span>
        <span className="font-medium text-sky-100">{toolName}</span>
        <span className="rounded bg-white/7 px-1.5 py-0.5 font-mono text-[10px] text-[var(--muted-foreground)]">
          {state}
        </span>
        <span className="ml-auto text-[11px] text-[var(--muted-foreground)] group-open:hidden">
          Show data
        </span>
        <span className="ml-auto hidden text-[11px] text-[var(--muted-foreground)] group-open:inline">
          Hide data
        </span>
      </summary>
      <div className="space-y-3 border-t border-sky-300/10 px-3 py-3">
        {details.map(([label, value]) =>
          value === undefined ? null : (
            <EventField key={label} label={label} value={value} />
          ),
        )}
        <details className="rounded border border-white/8 bg-black/15">
          <summary className="cursor-pointer px-2.5 py-2 font-mono text-[11px] text-[var(--muted-foreground)]">
            Raw UI message part
          </summary>
          <pre className="max-h-72 overflow-auto border-t border-white/8 p-2.5 text-[11px] leading-5 text-slate-300">
            {serializeEvent(part)}
          </pre>
        </details>
      </div>
    </details>
  );
}

function EventField({ label, value }: { label: string; value: unknown }) {
  return (
    <div>
      <p className="mb-1 font-mono text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--muted-foreground)]">
        {label}
      </p>
      <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded border border-white/8 bg-black/15 p-2.5 text-[11px] leading-5 text-slate-300">
        {serializeEvent(value)}
      </pre>
    </div>
  );
}

function serializeEvent(value: unknown): string {
  return JSON.stringify(value, null, 2) ?? String(value);
}

function StartupState({ message }: { message: string }) {
  return (
    <main className="flex h-screen items-center justify-center bg-[var(--background)] text-sm text-[var(--muted-foreground)]">
      {message}
    </main>
  );
}
