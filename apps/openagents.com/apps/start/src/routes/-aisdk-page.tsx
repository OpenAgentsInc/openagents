/**
 * /aisdk — the public page for the extracted OpenAgents AI SDK
 * (owner-directed addition, 2026-07-21). Content source:
 * `docs/ai-sdk/README.md` plus the published `@openagentsinc/ai` rc train
 * (https://github.com/OpenAgentsInc/ai). Reuses the `.oa-public-site`
 * design vocabulary — no new design language.
 */
import { InternalLink } from '@/components/internal-link'

import { PublicSiteShell } from './-public-site'
import './-aisdk.css'

export const AISDK_GITHUB_URL = 'https://github.com/OpenAgentsInc/ai'

const npmUrl = (name: string) => `https://www.npmjs.com/package/${name}`

const layers = [
  {
    id: 'L6',
    name: 'Recall',
    packages: '@openagentsinc/history-corpus · @openagentsinc/rlm',
    body: 'Corpus export, cursor-addressed entries, HistoryRecall, Tier D deterministic recall, and the recursive RLM engine.',
  },
  {
    id: 'L5',
    name: 'UI stream',
    packages: 'agent-harness-contract: ui-message-chunk · ui-message-reducer · smooth-stream · partial-object-stream · chat-transport',
    body: 'Redaction-aware projection from runtime events to renderable chunks, one reducer to a UiMessage, and event-log/SSE/IPC transports.',
  },
  {
    id: 'L4',
    name: 'Harness',
    packages: 'agent-harness-contract: AgentHarness adapter',
    body: 'Session verbs, capability-by-method-presence, slice runner, readiness projection, skills, host tools, and ACP + opencode adapters.',
  },
  {
    id: 'L3',
    name: 'Sandbox',
    packages: '@openagentsinc/ai-sdk-sandbox-local · @openagentsinc/ai-sdk-sandbox-openagents',
    body: 'The harness sandbox-provider contract, the local-process provider, and the managed OpenAgents provider.',
  },
  {
    id: 'L2',
    name: 'Durable log',
    packages: 'agent-harness-contract: event-log · event-log-store',
    body: 'Seq-cursor append, replay, live attach, and rerun boundaries. The stream survives the process.',
  },
  {
    id: 'L1',
    name: 'Vocabulary',
    packages: '@openagentsinc/agent-runtime-schema',
    body: 'KhalaRuntimeEvent — one neutral event union where sequence is the durable cursor and visibility + redactionClass + causalityRefs are schema fields.',
  },
  {
    id: 'L0',
    name: 'Model call',
    packages: 'effect/unstable/ai + @openagentsinc/ai-model',
    body: 'The upstream Effect AI LanguageModel Layer (consumed, never forked), bidirectional StreamPart maps, and ExecutionPlan fallback.',
  },
] as const

const roster = [
  ['@openagentsinc/ai', 'Umbrella', 'Curated re-exports of every layer entry point.'],
  ['@openagentsinc/agent-runtime-schema', 'L1 vocabulary', 'The neutral KhalaRuntimeEvent union and its codecs.'],
  ['@openagentsinc/agent-harness-contract', 'L2–L5 core', 'Durable log, sandbox contract, harness adapters, UI stream.'],
  ['@openagentsinc/ai-model', 'L0 bridge', 'LanguageModel Layer, StreamPart maps, fallback plans.'],
  ['@openagentsinc/history-corpus', 'L6 recall', 'Corpus builder, Tier D recall, history_recall host tool.'],
  ['@openagentsinc/rlm', 'RLM engine', 'Recursive recall programs over a corpus source.'],
  ['@openagentsinc/ai-sdk-sandbox-local', 'L3 provider', 'Isolated local account homes for harness work.'],
  ['@openagentsinc/ai-sdk-sandbox-openagents', 'L3 provider', 'Managed OpenAgents sandbox sessions.'],
] as const

const differentiators = [
  ['Durable cursor-exact streams', 'Attach, replay, and rerun boundaries over a persisted seq-cursor log, not a best-effort in-memory bridge.'],
  ['Suspend and continue that persists', 'A turn freezes at an exact cursor and resumes in a different process with no gap and no duplicate.'],
  ['Coding-agent harnesses', 'Codex, Claude Code, ACP peers, and opencode as adapters behind one versioned contract with capability-by-method-presence.'],
  ['Redaction as a schema field', 'visibility and redactionClass gate every projection, so a public surface cannot widen what it sees.'],
  ['Recall instead of compaction', 'The full history stays durable and a typed recall service traverses it, deterministically first and recursively second.'],
  ['Honest failure vocabulary', 'Typed model errors map onto operator-facing failure classes, and a fallback never launders an exhausted account.'],
] as const

const quickstartCode = `import { Effect, Stream } from 'effect'
import {
  applyUiChunk,
  initialUiMessage,
  khalaEventToUiChunks,
  makeReferenceAdapter,
} from '@openagentsinc/ai'

const program = Effect.gen(function* () {
  const harness = makeReferenceAdapter({ scriptWords: ['Hello ', 'world'] })
  const session = yield* harness.start({
    sessionId: 'session-1',
    source: { lane: 'test_fixture' },
  })
  const control = yield* session.promptTurn({ turnId: 'turn-1', prompt: 'Greet.' })
  const events = yield* Stream.runCollect(control.events)

  // KhalaRuntimeEvent -> UI chunks -> one reduced UiMessage
  const chunks = events.flatMap(event => khalaEventToUiChunks(event))
  return chunks.reduce(applyUiChunk, initialUiMessage())
})

const message = await Effect.runPromise(program)
// message.status === 'complete', one text part: 'Hello world'`

export function AisdkPage() {
  return (
    <div data-route="aisdk">
      <PublicSiteShell>
        <section className="oa-hero">
          <div className="oa-container oa-hero-inner oa-aisdk-hero-inner">
            <div className="oa-hero-copy">
              <p className="oa-kicker">The OpenAgents AI SDK</p>
              <h1>One event union.<br />One durable cursor.</h1>
              <p className="oa-hero-summary">
                The OpenAgents AI SDK is an Effect-native toolkit for building agent
                applications with durable, cursor-exact streams — one neutral event union
                from the model call to the rendered message, with suspend and continue that
                actually persists, coding-agent harnesses, redaction as a schema field, and
                recall instead of compaction.
              </p>
              <div className="oa-actions">
                <InternalLink className="oa-button oa-button-primary" href="/aisdk/docs" preload="render">Read the docs</InternalLink>
                <a className="oa-button oa-button-secondary" href={AISDK_GITHUB_URL} target="_blank" rel="noreferrer">View on GitHub ↗</a>
              </div>
              <p className="oa-aisdk-install"><code>pnpm add @openagentsinc/ai@rc</code></p>
              <p className="oa-release-note">Apache-2.0 · published as the @openagentsinc rc train on npm</p>
            </div>
          </div>
        </section>

        <section className="oa-aisdk-section" aria-labelledby="oa-aisdk-layers-title">
          <div className="oa-container">
            <div className="oa-section-heading">
              <div>
                <h2 id="oa-aisdk-layers-title">Seven layers, one rule.</h2>
                <p>Every layer speaks KhalaRuntimeEvent upward. L0 maps provider parts into it. L2 persists it. L4 emits it. L5 projects it to renderable chunks. L6 exports it to a corpus.</p>
              </div>
              <span>The layer diagram</span>
            </div>
            <div className="oa-aisdk-layer-stack">
              {layers.map(layer => (
                <div className="oa-aisdk-layer" key={layer.id}>
                  <span className="oa-aisdk-layer-id">{layer.id}</span>
                  <p className="oa-aisdk-layer-name">{layer.name}<span>{layer.packages}</span></p>
                  <p className="oa-aisdk-layer-body">{layer.body}</p>
                </div>
              ))}
              <p className="oa-aisdk-layer-rule">One event union. One durable cursor.</p>
            </div>
          </div>
        </section>

        <section className="oa-aisdk-section oa-aisdk-section-alt" aria-labelledby="oa-aisdk-roster-title">
          <div className="oa-container">
            <div className="oa-section-heading">
              <div>
                <h2 id="oa-aisdk-roster-title">The roster.</h2>
                <p>Eight packages, one repository, one rc train. Extracted from the OpenAgents monorepo and published from OpenAgentsInc/ai.</p>
              </div>
              <a href={AISDK_GITHUB_URL} target="_blank" rel="noreferrer">github.com/OpenAgentsInc/ai ↗</a>
            </div>
            <div className="oa-aisdk-table-wrap">
              <table className="oa-aisdk-table">
                <thead>
                  <tr><th>Package</th><th>Layer</th><th>Role</th></tr>
                </thead>
                <tbody>
                  {roster.map(([name, layer, role]) => (
                    <tr key={name}>
                      <td><a href={npmUrl(name)} target="_blank" rel="noreferrer"><code>{name}</code></a></td>
                      <td>{layer}</td>
                      <td>{role}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </section>

        <section className="oa-aisdk-section" aria-labelledby="oa-aisdk-diff-title">
          <div className="oa-container">
            <div className="oa-centered-heading">
              <p>What neither upstream offers</p>
              <h2 id="oa-aisdk-diff-title">Built for agents that outlive the process.</h2>
            </div>
            <div className="oa-aisdk-diff-grid">
              {differentiators.map(([title, body]) => (
                <article key={title}><h3>{title}</h3><p>{body}</p></article>
              ))}
            </div>
          </div>
        </section>

        <section className="oa-aisdk-section oa-aisdk-section-alt" aria-labelledby="oa-aisdk-quickstart-title">
          <div className="oa-container">
            <div className="oa-section-heading">
              <div>
                <h2 id="oa-aisdk-quickstart-title">Quickstart.</h2>
                <p>Install the rc train, run a harness turn, and reduce the stream to one message.</p>
              </div>
              <span>pnpm add @openagentsinc/ai@rc</span>
            </div>
            <div className="oa-aisdk-quickstart">
              <pre className="oa-aisdk-code"><code>{quickstartCode}</code></pre>
              <p className="oa-aisdk-quickstart-note">
                Every symbol above is exported by the published umbrella package. The packages
                ship TypeScript source and pin Effect 4.0.0-beta.94 — run with tsx, Vite, or
                another TypeScript-aware loader.
              </p>
            </div>
          </div>
        </section>

        <section className="oa-aisdk-section" aria-labelledby="oa-aisdk-docs-title">
          <div className="oa-container">
            <div className="oa-section-heading">
              <div>
                <h2 id="oa-aisdk-docs-title">Documentation.</h2>
                <p>Served from the Markdown kept in the repository under docs/ai-sdk.</p>
              </div>
              <InternalLink href="/aisdk/docs" preload="render">All docs →</InternalLink>
            </div>
            <div className="oa-aisdk-doc-links">
              <InternalLink href="/aisdk/docs" preload="render">
                <strong>Overview</strong>
                <span>Where the SDK lives, the current published train, and the consumption contract.</span>
              </InternalLink>
              <InternalLink href="/aisdk/docs/getting-started" preload="render">
                <strong>Getting started</strong>
                <span>Install the rc train and run three real programs against the published packages.</span>
              </InternalLink>
              <InternalLink href="/aisdk/docs/packages" preload="render">
                <strong>Packages</strong>
                <span>Every published package with its key exports and when to use it.</span>
              </InternalLink>
            </div>
          </div>
        </section>
      </PublicSiteShell>
    </div>
  )
}
