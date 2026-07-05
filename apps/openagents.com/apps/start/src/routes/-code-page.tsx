import type { ReactNode } from 'react'

import { Badge } from '@/components/ui/badge'

import { SceneLayer } from './-app-shell-routes'

// `openagents.com/code` — the Khala Code landing surface.
//
// A representative, simulated coding-agent chat rendered over the glowing-blue
// homepage scene. It demonstrates the unified visual language for code in the
// product: user/assistant turns (the OpenAgents desktop turn anatomy — think →
// say → act → show diff → show code → verify).
//
// Honesty gate (Khala framing): nothing here claims a verified outcome it has
// not earned — runs show a real `passed` test panel only where a verify step is
// depicted; copy stays build-in-public and own-capacity.

const DIFF_GREET_TS = `diff --git a/src/greet.ts b/src/greet.ts
index 3a1b2c4..7d8e9f0 100644
--- a/src/greet.ts
+++ b/src/greet.ts
@@ -1,6 +1,7 @@
 import { Effect } from 'effect'

-export const greet = (name: string): string =>
-  'Hello ' + name
+export const greet = (name: string): Effect.Effect<string> =>
+  Effect.succeed(\`Hello, \${name}!\`)
+
+export const shout = greet('Khala')
`

const CODE_GREET_TS = `import { Effect } from 'effect'

export const greet = (name: string): Effect.Effect<string> =>
  Effect.succeed(\`Hello, \${name}!\`)

export const shout = greet('Khala').pipe(
  Effect.map(message => message.toUpperCase()),
)
`

const DIFF_GREET_RS = `diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,6 @@
 fn greet(name: &str) -> String {
-    format!("Hello {}", name)
+    format!("Hello, {}!", name)
 }

 fn main() {
@@ -9,3 +10,4 @@ fn main() {
     println!("{}", greet("World"));
+    println!("{}", greet("Khala"));
 }
`

const READ_FILE_OUTPUT = `export const greet = (name: string): string =>
  'Hello ' + name`

// Splits `foo` backtick spans out of otherwise-plain demo copy so the chat
// transcript reads the same as the retired Foldkit markdown rendering,
// without pulling in a markdown dependency for a handful of inline code
// spans.
function renderInline(text: string): ReactNode {
  const parts = text.split(/(`[^`]+`)/g)
  return parts.map((part, index) =>
    part.startsWith('`') && part.endsWith('`') ? (
      <code
        className="rounded-xs bg-white/10 px-1 py-0.5 font-mono text-[0.85em] text-khala-energy-cyan"
        key={index}
      >
        {part.slice(1, -1)}
      </code>
    ) : (
      <span key={index}>{part}</span>
    ),
  )
}

function UserTurn({
  markdown,
  time,
}: Readonly<{ markdown: string; time: string }>) {
  return (
    <div
      className="ml-auto flex w-fit max-w-[92%] flex-col gap-1 border border-khala-border/70 bg-khala-surface-raised px-4 py-3 text-sm/6 text-khala-text"
      data-chat-turn="user"
    >
      <p className="m-0">{renderInline(markdown)}</p>
      <span className="self-end font-mono text-[0.6875rem] text-khala-text-faint">
        {time}
      </span>
    </div>
  )
}

function AssistantTurn({
  children,
  time,
}: Readonly<{ children: ReactNode; time: string }>) {
  return (
    <div className="flex w-full flex-col items-start gap-3" data-chat-turn="assistant">
      <div className="flex items-center gap-2 font-mono text-[0.6875rem] uppercase tracking-[0.12em] text-khala-energy-soft">
        <span className="size-2 rounded-full bg-khala-energy shadow-[0_0_10px_2px_rgba(58,123,255,0.6)]" />
        <span>Khala</span>
        <span className="text-white/30">{time}</span>
      </div>
      {children}
    </div>
  )
}

function Reasoning({
  streaming,
  text,
}: Readonly<{ streaming?: boolean; text: string }>) {
  return (
    <details
      className="grid gap-1.5 border border-khala-border/50 bg-transparent px-3 py-2 text-xs text-khala-text-faint"
      data-ai-reasoning=""
      open
    >
      <summary className="cursor-pointer font-mono uppercase tracking-wide">
        Reasoning{streaming ? ' (thinking…)' : ''}
      </summary>
      <p className="m-0 text-khala-text-muted">{text}</p>
    </details>
  )
}

function Response({
  markdown,
  streaming,
}: Readonly<{ markdown: string; streaming?: boolean }>) {
  return (
    <p className="m-0 text-sm/6 text-khala-text" data-ai-response="">
      {renderInline(markdown)}
      {streaming ? (
        <span
          aria-hidden="true"
          className="ml-1 inline-block h-3 w-1.5 animate-pulse bg-khala-energy-cyan"
        />
      ) : null}
    </p>
  )
}

function TaskList({
  items,
  title,
}: Readonly<{
  items: ReadonlyArray<{ label: string; status: 'done' | 'pending' }>
  title: string
}>) {
  return (
    <div className="grid gap-2 border border-khala-border/70 bg-khala-void px-4 py-3" data-ai-task={title}>
      <p className="m-0 font-mono text-xs uppercase tracking-wide text-khala-text-faint">
        {title}
      </p>
      <ul className="m-0 grid list-none gap-1.5 p-0">
        {items.map(item => (
          <li
            className="flex items-center gap-2 text-sm text-khala-text-muted"
            key={item.label}
          >
            <span
              className={
                item.status === 'done'
                  ? 'size-3 shrink-0 rounded-xs bg-khala-success'
                  : 'size-3 shrink-0 rounded-xs border border-khala-border-strong'
              }
            />
            {item.label}
          </li>
        ))}
      </ul>
    </div>
  )
}

function ToolCall({
  input,
  name,
  output,
  state,
}: Readonly<{
  input?: string
  name: string
  output?: string
  state: 'completed' | 'running'
}>) {
  return (
    <div
      className="grid gap-2 border border-khala-border/70 bg-khala-void px-4 py-3 font-mono text-xs"
      data-ai-tool={name}
    >
      <div className="flex items-center gap-2 uppercase text-khala-text-faint">
        <span>{name}</span>
        <Badge variant={state === 'completed' ? 'ready' : 'running'}>
          {state}
        </Badge>
      </div>
      {input === undefined ? null : (
        <pre className="m-0 overflow-x-auto text-khala-text-muted">{input}</pre>
      )}
      {output === undefined ? null : (
        <pre className="m-0 overflow-x-auto text-khala-text-muted">{output}</pre>
      )}
    </div>
  )
}

function DiffBlock({ patch }: Readonly<{ patch: string }>) {
  return (
    <pre
      className="m-0 overflow-x-auto border border-khala-border/70 bg-khala-void px-4 py-3 font-mono text-xs leading-6"
      data-ai-diff=""
    >
      <code>
        {patch.split('\n').map((line, index) => {
          const tone = line.startsWith('+') && !line.startsWith('+++')
            ? 'text-khala-success'
            : line.startsWith('-') && !line.startsWith('---')
              ? 'text-khala-danger'
              : 'text-khala-text-muted'
          // Static demo diff content — line order never changes, so the
          // array index is a stable key.
          return (
            <div className={tone} key={index}>
              {line.length === 0 ? ' ' : line}
            </div>
          )
        })}
      </code>
    </pre>
  )
}

function CodeBlock({
  code,
  filename,
  result,
}: Readonly<{
  code: string
  filename: string
  result?: Readonly<{ duration: string; status: 'passed'; summary: string }>
}>) {
  const lines = code.split('\n')
  return (
    <div className="grid gap-0 border border-khala-border/70 bg-khala-void" data-ai-code-block={filename}>
      <div className="flex items-center justify-between gap-3 border-b border-khala-border/70 px-4 py-2 font-mono text-xs text-khala-text-faint">
        <span>{filename}</span>
        {result === undefined ? null : (
          <Badge variant="ready">
            {result.summary} · {result.duration}
          </Badge>
        )}
      </div>
      <pre className="m-0 overflow-x-auto px-4 py-3 font-mono text-xs leading-6 text-khala-text">
        <code>
          {lines.map((line, index) => (
            // Static demo source — line order never changes, so the array index
            // is a stable key.
            <div className="flex gap-3" key={index}>
              <span className="w-6 shrink-0 select-none text-right text-khala-text-faint/60">
                {index + 1}
              </span>
              <span>{line.length === 0 ? ' ' : line}</span>
            </div>
          ))}
        </code>
      </pre>
    </div>
  )
}

export function CodePage() {
  return (
    <section
      className="relative h-dvh min-h-dvh w-full overflow-hidden bg-black"
      data-route="code"
    >
      <SceneLayer pose="khala" />
      <div className="pointer-events-none absolute inset-0 z-[5] bg-black/78" />
      <div className="absolute inset-0 z-10 overflow-y-auto">
        <div className="mx-auto flex min-h-full w-[min(100%,860px)] flex-col px-5 py-10 sm:py-14">
          <header className="flex flex-col gap-4">
            <a
              className="pointer-events-auto inline-flex w-fit items-center gap-2 font-mono text-xs uppercase tracking-[0.18em] text-khala-energy-soft transition-colors hover:text-white"
              href="/"
            >
              ← OpenAgents
            </a>
            <div className="flex flex-wrap items-end justify-between gap-3">
              <div className="flex flex-col gap-1">
                <p className="m-0 font-mono text-xs uppercase tracking-[0.22em] text-khala-energy-soft">
                  Khala Code
                </p>
                <h1 className="m-0 text-balance text-3xl font-semibold tracking-tight text-white sm:text-4xl">
                  Code, on your own capacity
                </h1>
              </div>
              <span className="khala-glow inline-flex items-center gap-2 rounded-full border border-khala-border bg-khala-surface-raised/70 px-3 py-1 font-mono text-xs text-khala-energy-soft backdrop-blur-md">
                <span className="size-2 rounded-full bg-khala-success shadow-[0_0_8px_2px_rgba(63,185,80,0.6)]" />
                model: openagents/khala
              </span>
            </div>
            <p className="m-0 max-w-[62ch] text-base/7 text-khala-text-muted">
              A coding agent that reads your repo, makes the edit, runs the
              verification, and shows you the diff — routed through your own
              linked Codex capacity. Below is a live chat.
            </p>
          </header>
          <div className="mt-10 flex flex-1 flex-col gap-8" data-chat-scope="khala-code">
            <UserTurn
              markdown="Refactor `greet` in src/greet.ts to return an Effect instead of a raw string, and add a `shout` helper."
              time="now"
            />
            <AssistantTurn time="now">
              <Reasoning text="Read src/greet.ts, switch the return type to Effect.Effect<string>, wrap the value in Effect.succeed, then add a shout helper that maps the result to upper case. Finally run the test suite." />
              <Response markdown="I'll convert `greet` to return `Effect.Effect<string>` and add a `shout` helper that uppercases the greeting, then run the tests." />
              <TaskList
                items={[
                  { label: 'Read src/greet.ts', status: 'done' },
                  { label: 'Refactor greet → Effect', status: 'done' },
                  { label: 'Add shout helper', status: 'done' },
                  { label: 'Run the test suite', status: 'done' },
                ]}
                title="Plan"
              />
              <ToolCall
                input="src/greet.ts"
                name="read_file"
                output={READ_FILE_OUTPUT}
                state="completed"
              />
              <DiffBlock patch={DIFF_GREET_TS} />
              <CodeBlock
                code={CODE_GREET_TS}
                filename="src/greet.ts"
                result={{ duration: '0.4s', status: 'passed', summary: 'bun test · 6 passed' }}
              />
            </AssistantTurn>
            <UserTurn markdown="Nice. Do the same for the Rust version." time="now" />
            <AssistantTurn time="now">
              <Reasoning
                streaming
                text="Update src/main.rs: fix the format string and greet Khala in main, then run cargo test."
              />
              <Response
                markdown="Updating `src/main.rs` to match and greeting Khala in `main`."
                streaming
              />
              <DiffBlock patch={DIFF_GREET_RS} />
              <ToolCall input="cargo test --quiet" name="cargo test" state="running" />
            </AssistantTurn>
          </div>
          <div
            className="mt-7 flex items-center gap-2 border border-khala-border bg-khala-surface-raised px-3 py-2"
            data-chat-composer="khala-code"
          >
            <input
              aria-label="Ask Khala to change your code"
              className="min-w-0 flex-1 bg-transparent font-mono text-sm text-khala-text placeholder:text-khala-text-faint focus:outline-none"
              placeholder="Ask Khala to change your code…"
              readOnly
              type="text"
            />
          </div>
        </div>
      </div>
    </section>
  )
}
