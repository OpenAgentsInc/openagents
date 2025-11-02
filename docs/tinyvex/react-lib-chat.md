Yes. Let’s standardize this the Convex way: one tiny, well-typed React hook that any app can drop in to consume Tinyvex (your “TinyFAX”) with zero app-specific logic.

Below is a complete plan + skeleton you can hand to the coding agent.

---

# Deliverable

Create a new package: **`packages/tvx-react`** (and a core transport **`packages/tvx-client`**).

It exposes exactly **one primary hook** for chat threads:

```ts
const {
  status,            // 'connecting' | 'ready' | 'error'
  threadId,          // canonical, resolved once
  history,           // final, persisted messages (ACP: kind='message', role set)
  live,              // ephemeral { assistant: string; thought?: string }
  send,              // (text: string, opts?: { resumeId?: 'last'; provider?: string }) => Promise<void>
  refresh,           // () => void
  debug,             // counters & last events, behind a flag
} = useTvxThread({ idOrAlias: string });
```

Everything else (aliases, WS resubscribe, dedupe, partials, etc.) is **inside** this hook. App code becomes dumb.

---

# Requirements (ADR-compliant)

* **ADR-0007 (ACP):**
  Persist **final** messages only (`kind='message'` + `role`). Chunks are **ephemeral** to `live`.
* **ADR-0002 (Rust→TS):**
  Use **generated** types (`MessageRowTs`, `ThreadSummaryTs`, etc.). **No `any`**.
* **Identity:**
  Hook starts by calling `threads.resolve({ id_or_alias })` and uses the **canonical** `thread_id` for all queries/subs.
* **Determinism:**
  Clock + IdGen injectable in the client (for tests).

---

# Package layout

```
packages/
  tvx-client/              # framework-agnostic transport + state machine
    src/
      Transport.ts         # WS adapter interface
      TvxClient.ts         # resolve, query, subscribe, send
      events.ts            # typed union of bridge events (generated TS)
      model.ts             # ACP models (generated TS)
      aggregator.ts        # live chunk aggregator (assistant/thought)
      identity.ts          # threads.resolve + alias→canonical
      dedupe.ts            # stable message key
      logger.ts            # pluggable logger (counters)
      index.ts
    tests/...

  tvx-react/
    src/
      useTvxThread.ts      # the Hook (wraps TvxClient)
      Provider.tsx         # <TvxProvider config={...}>
      index.ts
    tests/...
```

---

# Transport & client (core, framework-agnostic)

```ts
// packages/tvx-client/src/Transport.ts
export interface Transport {
  connect(): Promise<void>;
  close(): void;
  send(control: { name: string; args?: unknown }): void;
  onMessage(cb: (evt: unknown) => void): () => void;
  status(): 'connecting'|'open'|'closed'|'error';
}

// packages/tvx-client/src/TvxClient.ts
import { Transport } from './Transport';
import { MessageRowTs } from './model';
import { resolveAlias } from './identity';
import { Dedupe } from './dedupe';
import { LiveAggregator } from './aggregator';

export class TvxClient {
  constructor(private t: Transport, private log = console) {}

  async init(idOrAlias: string) {
    await this.t.connect();
    const { thread_id } = await this.rpc('threads.resolve', { id_or_alias: idOrAlias });
    this.subscribeThread(thread_id);
    this.queryHistory(thread_id);
    return thread_id;
  }

  private subscribeThread(threadId: string) {
    this.t.send({ name: 'tvx.subscribe', args: { stream: 'messages', thread_id: threadId }});
  }
  private queryHistory(threadId: string) {
    this.t.send({ name: 'tvx.query', args: { name: 'messages.list', args: { thread_id: threadId, limit: 500 } }});
  }

  onEvents(handlers: {
    history: (rows: MessageRowTs[]) => void;
    live: (ev: { assistant?: string; thought?: string }) => void;
    debug?: (s: unknown) => void;
  }) {
    const dedupe = new Dedupe();
    const live = new LiveAggregator();
    return this.t.onMessage((evt: any) => {
      switch (evt.type) {
        case 'tinyvex.snapshot':
        case 'tinyvex.query_result': {
          const rows = (evt.rows as MessageRowTs[]).filter(r => r.kind === 'message' && !!r.role);
          handlers.history(dedupe.merge(rows));
          handlers.debug?.({ tag: 'history', kept: rows.length, totals: dedupe.stats() });
          break;
        }
        case 'tinyvex.update': {
          const rows = (evt.rows as MessageRowTs[]).filter(r => r.kind === 'message' && !!r.role);
          handlers.history(dedupe.merge(rows));
          handlers.debug?.({ tag: 'update', kept: rows.length, totals: dedupe.stats() });
          break;
        }
        // Ephemeral ACP live
        case 'agent_message_live': {
          live.appendAssistant(evt.delta ?? '');
          handlers.live({ assistant: live.assistant() });
          break;
        }
        case 'agent_thought_live': {
          live.appendThought(evt.delta ?? '');
          handlers.live({ thought: live.thought() });
          break;
        }
        case 'agent_message_done': {
          live.clearAssistant();
          handlers.live({ assistant: '' });
          break;
        }
      }
    });
  }

  async send(text: string, opts?: { resumeId?: 'last'; provider?: string }) {
    this.t.send({ name: 'run.submit', args: { text, resume_id: opts?.resumeId ?? 'last', provider: opts?.provider }});
  }

  private rpc<T=any>(name: string, args?: any): Promise<T> {
    // minimal request/response implementation (or reuse your WS RPC)
    // …
    return Promise.reject(new Error('not implemented in sketch'));
  }
}
```

**Notes**

* `Dedupe.merge()` uses a stable `message_id` (or deterministic fallback).
* `LiveAggregator` is in-memory only. Persisted history is final only.

---

# The React hook

```ts
// packages/tvx-react/src/Provider.tsx
export type TvxConfig = { url: string; token?: string; debug?: boolean };
const Ctx = React.createContext<TvxClient | null>(null);

export function TvxProvider({ config, children }: { config: TvxConfig; children: React.ReactNode }) {
  const client = React.useMemo(() => makeClient(config), [config.url, config.token]);
  return <Ctx.Provider value={client}>{children}</Ctx.Provider>;
}
function makeClient(cfg: TvxConfig) {
  const transport = new WsTransport(cfg); // implements Transport
  return new TvxClient(transport, cfg.debug ? console : silentLogger);
}

// packages/tvx-react/src/useTvxThread.ts
import { useEffect, useMemo, useReducer, useContext } from 'react';
import { MessageRowTs } from '@openagents/tvx-client/model';

type State = {
  status: 'idle'|'connecting'|'ready'|'error';
  threadId?: string;
  history: MessageRowTs[];
  live: { assistant: string; thought?: string };
  debug?: { kept: number; duplicate: number; nonMessageKind: number };
  error?: string;
};

export function useTvxThread({ idOrAlias }: { idOrAlias: string }) {
  const client = useContext(Ctx)!;
  const [st, dispatch] = useReducer(reducer, { status: 'connecting', history: [], live: { assistant: '' } });

  useEffect(() => {
    let off = () => {};
    (async () => {
      try {
        const threadId = await client.init(idOrAlias);
        dispatch({ type: 'resolved', threadId });
        off = client.onEvents({
          history: rows => dispatch({ type: 'history', rows }),
          live: ev => dispatch({ type: 'live', ev }),
          debug: d => dispatch({ type: 'debug', d }),
        });
        dispatch({ type: 'ready' });
      } catch (e: any) {
        dispatch({ type: 'error', error: String(e?.message ?? e) });
      }
    })();
    return () => off();
  }, [client, idOrAlias]);

  const send = (text: string, opts?: { resumeId?: 'last'; provider?: string }) => client.send(text, opts);
  const refresh = () => st.threadId && client['queryHistory']?.(st.threadId);

  return { status: st.status, threadId: st.threadId, history: st.history, live: st.live, send, refresh, debug: st.debug };
}

function reducer(state: State, action: any): State {
  switch (action.type) {
    case 'resolved': return { ...state, threadId: action.threadId };
    case 'ready':    return { ...state, status: 'ready' };
    case 'history':  return { ...state, history: action.rows };
    case 'live':     return { ...state, live: { ...state.live, ...action.ev } };
    case 'debug':    return { ...state, debug: summarize(action.d) };
    case 'error':    return { ...state, status: 'error', error: action.error };
    default:         return state;
  }
}
```

**App usage (Expo)**

```tsx
import { TvxProvider, useTvxThread } from '@openagents/tvx-react';

<TvxProvider config={{ url: WS_URL, token: TOKEN, debug: true }}>
  <ThreadScreen idOrAlias={params.id} />
</TvxProvider>

function ThreadScreen({ idOrAlias }) {
  const { status, history, live, send, debug } = useTvxThread({ idOrAlias });
  // render history bubbles, optional live bubble from `live.assistant`
}
```

---

# Move logic out of the app

* Delete the custom `use-thread-timeline.tsx` and helpers that:

  * merged multiple thread IDs,
  * guessed roles from `kind`,
  * handled partials,
  * performed dedupe,
  * wired ad-hoc logging.
* Replace with the single hook above.

---

# Tests

**tvx-client (unit)**

* Aggregator: any chunk sequence + done → single final persisted row (idempotent on duplicate done).
* Dedupe: same `message_id` twice → one item.

**tvx-react (react)**

* Render with a mock `Transport`:

  * chunks → live text updates; on `agent_message_done` → live clears; on `tinyvex.update` → history +1.
* Reconnect path: `Transport` close/open retains cached `threadId`, resubscribes once.

**Maestro (E2E)**

* “Send → stream → finalize”: one live bubble grows, one history bubble committed, no duplicates after reconnect.

---

# Acceptance criteria (for the PR)

* **One hook** (`useTvxThread`) used by the Expo app; all timeline logic removed from app code.
* Hook **resolves alias → canonical**, subscribes & queries **once**.
* **No partials** in persisted history; `history` only contains ACP `message` rows with `role`.
* **Live streaming** updates come through `live.assistant`; finalization clears it.
* **No `any`** on the hook path; all types imported from generated `ts-rs`.
* Deterministic tests pass with fake transport and fixed clock/id.

---

If you want this even closer to Convex ergonomics, add `useTvxQuery(name,args)` and `useTvxMutation(name)` later. But for threads, **keep the surface area to one hook**—the above covers identity, streaming, persistence, and sending in a single, drop-in API.
