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

const glassPanelClass =
  'relative overflow-hidden rounded-[1.25rem] border border-[#17b9ff]/35 bg-[#061023]/70 shadow-[0_24px_80px_rgba(0,0,0,0.46),inset_0_1px_0_rgba(255,255,255,0.14)] backdrop-blur-xl'

const neonPillClass =
  'inline-flex items-center justify-center rounded-full border border-white/35 bg-[linear-gradient(180deg,rgba(0,215,255,0.95)_0%,rgba(2,109,255,0.82)_48%,rgba(9,15,38,0.92)_100%)] px-4 py-2 font-mono text-[0.75rem] font-semibold uppercase text-white shadow-[0_14px_34px_rgba(2,109,255,0.35),inset_0_1px_0_rgba(255,255,255,0.45)]'

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
          'A coding agent that reads your repo, makes the edit, runs the verification, and shows you the diff. The current desktop path wraps your own local Codex install; the public install details keep that requirement visible.',
        ],
      ),
      h.div(
        [Ui.className<Message>('flex flex-wrap gap-3 pt-1')],
        [
          h.a(
            [
              h.Href('/code/download'),
              Ui.className<Message>(neonPillClass),
            ],
            ['Install paths'],
          ),
          h.a(
            [
              h.Href('/docs/openagents'),
              Ui.className<Message>(
                'inline-flex items-center justify-center rounded-full border border-white/15 bg-white/[0.04] px-4 py-2 font-mono text-[0.75rem] font-semibold uppercase text-[#cdeeff] transition-colors hover:border-[#4fd0ff]/60 hover:text-white',
              ),
            ],
            ['Read the contract'],
          ),
        ],
      ),
    ],
  )
}

const dialMetricView = <Message>(label: string, value: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid min-h-[4.25rem] content-center gap-1 border-[#17b9ff]/15 px-4 py-3',
      ),
    ],
    [
      h.strong(
        [Ui.className<Message>('font-mono text-xl leading-none text-white')],
        [value],
      ),
      h.span(
        [Ui.className<Message>('text-sm leading-none text-white/55')],
        [label],
      ),
    ],
  )
}

// Not currently rendered on the public route — the metrics inside were
// hardcoded placeholders (fabricated "turns ready" / "queued" / "verified"
// counts), not real state. Kept here, unwired, for a future pass that backs
// it with a real capacity/turn projection instead of static strings.
export const controlPreviewView = <Message>(): Html => {
  const h = html<Message>()

  return h.aside(
    [
      h.AriaLabel('Khala Code control preview'),
      Ui.className<Message>(
        'relative mx-auto grid w-full max-w-[26rem] content-start gap-5 rounded-[2rem] border border-[#1d3a6a] bg-[#080b1c] p-5 shadow-[0_30px_90px_rgba(0,0,0,0.58)] sm:p-6 lg:sticky lg:top-10',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'pointer-events-none absolute -left-32 -top-28 h-72 w-72 rounded-full bg-[#026dff]/45 blur-[72px]',
          ),
        ],
        [],
      ),
      h.div(
        [
          Ui.className<Message>(
            'pointer-events-none absolute -right-20 top-24 h-56 w-56 rounded-full bg-[#00d7ff]/25 blur-[64px]',
          ),
        ],
        [],
      ),
      h.div(
        [Ui.className<Message>('relative z-10 flex items-center justify-between gap-4')],
        [
          h.div([Ui.className<Message>('grid gap-1')], [
            h.p(
              [
                Ui.className<Message>(
                  'm-0 font-mono text-[0.68rem] uppercase tracking-[0.2em] text-[#59b4f7]',
                ),
              ],
              ['Local harness'],
            ),
            h.h2(
              [Ui.className<Message>('m-0 text-xl font-semibold text-white')],
              ['Khala Code'],
            ),
          ]),
          h.span(
            [
              Ui.className<Message>(
                'rounded-full border border-white/20 bg-white/[0.04] px-3 py-1 font-mono text-[0.68rem] uppercase text-white/60',
              ),
            ],
            ['Codex required'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            `${glassPanelClass} relative z-10 grid aspect-square place-items-center`,
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'absolute inset-6 rounded-full border border-[#1d4d95]/65 bg-[conic-gradient(from_212deg,rgba(0,215,255,0)_0deg,rgba(0,215,255,0.9)_76deg,rgba(2,109,255,0.92)_132deg,rgba(23,185,255,0.08)_178deg,rgba(255,255,255,0.04)_360deg)] shadow-[0_0_48px_rgba(2,109,255,0.35)]',
              ),
            ],
            [],
          ),
          h.div(
            [
              Ui.className<Message>(
                'absolute inset-12 rounded-full border border-white/10 bg-[#09132b]/80 shadow-[inset_0_0_42px_rgba(0,215,255,0.24)]',
              ),
            ],
            [],
          ),
          h.div(
            [
              Ui.className<Message>(
                'absolute right-[22%] top-[22%] size-7 rounded-full border border-white/40 bg-[linear-gradient(145deg,#00d7ff,#026dff)] shadow-[0_0_24px_rgba(0,215,255,0.75)]',
              ),
            ],
            [],
          ),
          h.div(
            [Ui.className<Message>('relative z-10 grid place-items-center gap-1 text-center')],
            [
              h.strong(
                [Ui.className<Message>('font-mono text-4xl leading-none text-white')],
                ['23'],
              ),
              h.span(
                [
                  Ui.className<Message>(
                    'font-mono text-[0.7rem] uppercase tracking-[0.2em] text-[#8fd4ff]',
                  ),
                ],
                ['turns ready'],
              ),
            ],
          ),
          h.span(
            [Ui.className<Message>('absolute left-5 top-1/2 font-mono text-xs text-white/45')],
            ['10'],
          ),
          h.span(
            [Ui.className<Message>('absolute right-5 top-1/2 font-mono text-xs text-white/45')],
            ['30'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            `${glassPanelClass} relative z-10 grid grid-cols-2 divide-x divide-y divide-[#17b9ff]/15`,
          ),
        ],
        [
          dialMetricView<Message>('queued', '4'),
          dialMetricView<Message>('ready', '2'),
          dialMetricView<Message>('verified', '6'),
          dialMetricView<Message>('tokens', '18k'),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'relative z-10 grid gap-3 rounded-[1.25rem] border border-[#17b9ff]/25 bg-[#061023]/70 p-4',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('flex items-center justify-between gap-3')],
            [
              h.span([Ui.className<Message>('font-mono text-sm text-white')], ['Local Codex']),
              h.span(
                [
                  Ui.className<Message>(
                    'relative h-8 w-14 rounded-full border border-white/20 bg-[#111a34]',
                  ),
                ],
                [
                  h.span(
                    [
                      Ui.className<Message>(
                        'absolute right-1 top-1 size-6 rounded-full border border-white/40 bg-[linear-gradient(145deg,#00d7ff,#026dff)] shadow-[0_0_14px_rgba(0,215,255,0.55)]',
                      ),
                    ],
                    [],
                  ),
                ],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('flex items-center justify-between gap-3')],
            [
              h.span([Ui.className<Message>('font-mono text-sm text-white')], ['Proof panel']),
              h.span(
                [Ui.className<Message>('font-mono text-xs uppercase text-[#8fd4ff]')],
                ['armed'],
              ),
            ],
          ),
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
