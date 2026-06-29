import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { landingSquaresView } from '../scene/landingSquaresElement'
import * as Ui from '../ui'
import { codeCopyScopeView } from '../ui/codeCopyScopeElement'
import type { PublicHeaderAuthState } from './publicHeader'

// `openagents.com/code` — the Khala Code landing surface.
//
// A representative, simulated coding-agent chat rendered over the glowing-blue
// homepage scene. It demonstrates the unified visual language for code in the
// product: user/assistant turns (the OpenAgents desktop turn anatomy — think →
// say → act → show diff → show code → verify), composed from the shared
// `@openagentsinc/ui` ai-elements (message, response, reasoning, task, tool,
// code block, diff, prompt input). The same components back the desktop chat,
// so this page doubles as the canonical reference for that language.
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

// A user turn: a right-aligned dark bubble via the shared message component.
const userTurn = <Message>(time: string, markdown: string): Html =>
  Ui.AiElements.message<Message>({
    props: { role: 'user', author: 'You', time },
    markdown,
  })

// An assistant turn: an open, centered column (no outer bubble) so the embedded
// code/diff/tool cards breathe — mirroring the desktop assistant-turn anatomy.
const assistantTurn = <Message>(input: {
  time: string
  parts: ReadonlyArray<Html>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('chat-turn', 'assistant'),
      Ui.className<Message>('flex w-full flex-col items-start gap-3'),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex items-center gap-2 font-mono text-[0.6875rem] uppercase tracking-[0.12em] text-[#8fb6ff]',
          ),
        ],
        [
          h.span([Ui.className<Message>('size-2 rounded-full bg-[#3a7bff] shadow-[0_0_10px_2px_rgba(58,123,255,0.6)]')], []),
          h.span([], ['Khala']),
          h.span([Ui.className<Message>('text-white/30')], [input.time]),
        ],
      ),
      ...input.parts,
    ],
  )
}

const headerView = <Message>(): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('flex flex-col gap-4')],
    [
      h.a(
        [
          h.Href('/'),
          Ui.className<Message>(
            'pointer-events-auto inline-flex w-fit items-center gap-2 font-mono text-[0.75rem] uppercase tracking-[0.18em] text-[#8fb6ff] transition-colors hover:text-white',
          ),
        ],
        ['← OpenAgents'],
      ),
      h.div(
        [Ui.className<Message>('flex flex-wrap items-end justify-between gap-3')],
        [
          h.div(
            [Ui.className<Message>('flex flex-col gap-1')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 font-mono text-[0.75rem] uppercase tracking-[0.22em] text-[#7aa2ff]',
                  ),
                ],
                ['Khala Code'],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 text-balance font-semibold tracking-tight text-white text-3xl sm:text-4xl',
                  ),
                ],
                ['Code, on your own capacity'],
              ),
            ],
          ),
          h.span(
            [
              Ui.className<Message>(
                'khala-glow inline-flex items-center gap-2 rounded-full border border-[#27406b] bg-[#0b1322]/70 px-3 py-1 font-mono text-[0.75rem] text-[#aecbff] backdrop-blur-md',
              ),
            ],
            [
              h.span([Ui.className<Message>('size-2 rounded-full bg-[#3fb950] shadow-[0_0_8px_2px_rgba(63,185,80,0.6)]')], []),
              h.span([], ['model: openagents/khala']),
            ],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('m-0 max-w-[62ch] text-base/7 text-[#c9d2dd]')],
        [
          'A coding agent that reads your repo, makes the edit, runs the ' +
            'verification, and shows you the diff — routed through your own ' +
            'linked Codex capacity. Below is a live chat.',
        ],
      ),
    ],
  )
}

const composerView = <Message>(): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('chat-composer', 'khala-code'),
      Ui.className<Message>('mt-7'),
    ],
    [
      Ui.AiElements.promptInput<Message>({
        props: {
          name: 'prompt',
          placeholder: 'Ask Khala to change your code…',
          status: 'ready',
          submitLabel: 'Send',
          rows: 2,
        },
      }),
    ],
  )
}

export const view = <Message>(
  _authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()
  const A = Ui.AiElements

  const conversation: ReadonlyArray<Html> = [
    userTurn<Message>(
      'now',
      'Refactor `greet` in src/greet.ts to return an Effect instead of a raw string, and add a `shout` helper.',
    ),
    assistantTurn<Message>({
      time: 'now',
      parts: [
        A.reasoning<Message>({
          props: {
            text: 'Read src/greet.ts, switch the return type to Effect.Effect<string>, wrap the value in Effect.succeed, then add a shout helper that maps the result to upper case. Finally run the test suite.',
            duration: 3,
          },
        }),
        A.response<Message>({
          markdown:
            "I'll convert `greet` to return `Effect.Effect<string>` and add a `shout` helper that uppercases the greeting, then run the tests.",
        }),
        A.task<Message>({
          props: {
            title: 'Plan',
            open: true,
            items: [
              { label: 'Read src/greet.ts', status: 'done' },
              { label: 'Refactor greet → Effect', status: 'done' },
              { label: 'Add shout helper', status: 'done' },
              { label: 'Run the test suite', status: 'done' },
            ],
          },
        }),
        A.tool<Message>({
          props: {
            name: 'read_file',
            state: 'completed',
            input: 'src/greet.ts',
            output: READ_FILE_OUTPUT,
          },
        }),
        A.diff<Message>({
          props: { patch: DIFF_GREET_TS, language: 'typescript' },
        }),
        A.codeBlock<Message>({
          props: {
            code: CODE_GREET_TS,
            language: 'typescript',
            filename: 'src/greet.ts',
          },
          showLineNumbers: true,
          result: {
            status: 'passed',
            summary: 'bun test · 6 passed',
            duration: '0.4s',
          },
        }),
      ],
    }),
    userTurn<Message>('now', 'Nice. Do the same for the Rust version.'),
    assistantTurn<Message>({
      time: 'now',
      parts: [
        A.reasoning<Message>({
          props: { text: 'Update src/main.rs: fix the format string and greet Khala in main, then run cargo test.', streaming: true },
        }),
        A.response<Message>({
          markdown: 'Updating `src/main.rs` to match and greeting Khala in `main`.',
          streaming: true,
        }),
        A.diff<Message>({
          props: { patch: DIFF_GREET_RS, language: 'rust' },
        }),
        A.tool<Message>({
          props: {
            name: 'cargo test',
            state: 'running',
            input: 'cargo test --quiet',
          },
        }),
      ],
    }),
  ]

  return h.div(
    [
      h.DataAttribute('route', 'code'),
      Ui.className<Message>(
        'relative h-screen h-dvh min-h-screen min-h-dvh w-full overflow-hidden bg-black',
      ),
    ],
    [
      landingSquaresView<Message>([
        Ui.className<Message>('block'),
        h.DataAttribute('pose', 'khala'),
      ]),
      h.div(
        [
          Ui.className<Message>(
            'pointer-events-none absolute inset-0 z-[5] bg-black/78',
          ),
        ],
        [],
      ),
      h.div(
        [Ui.className<Message>('absolute inset-0 z-10 overflow-y-auto')],
        [
          h.div(
            [
              Ui.className<Message>(
                'mx-auto flex min-h-full w-[min(100%,860px)] flex-col px-5 py-10 sm:py-14',
              ),
            ],
            [
              headerView<Message>(),
              codeCopyScopeView<Message>(
                [Ui.className<Message>('mt-10 flex flex-1 flex-col gap-8')],
                conversation,
              ),
              composerView<Message>(),
            ],
          ),
        ],
      ),
    ],
  )
}
