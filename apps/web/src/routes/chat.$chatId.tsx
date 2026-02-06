import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { DotsBackground, whitePreset } from '@openagentsinc/hud/react';
import { useAgent } from 'agents/react';
import { useAgentChat } from '@cloudflare/ai-chat/react';
import { Effect } from 'effect';
import { useMemo, useState } from 'react';
import { TelemetryService } from '../effect/telemetry';
import type { UIMessage } from 'ai';
import type { FormEvent } from 'react';

export const Route = createFileRoute('/chat/$chatId')({
  loader: async ({ context, params }) => {
    const result = await context.effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;

        const auth = yield* Effect.tryPromise({
          try: () => getAuth(),
          catch: (err) => err,
        }).pipe(Effect.catchAll(() => Effect.succeed(null)));

        const userId = auth?.user?.id ?? null;
        if (!userId) {
          yield* telemetry.withNamespace('route.chat').event('chat.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        if (params.chatId !== userId) {
          yield* telemetry.withNamespace('route.chat').event('chat.mismatch', {
            userId,
            chatId: params.chatId,
          });
          return { kind: 'redirect' as const, to: '/assistant' as const };
        }

        yield* telemetry.withNamespace('route.chat').event('chat.open', {
          userId,
          chatId: params.chatId,
        });

        return { kind: 'ok' as const, userId };
      }),
    );

    if (result.kind === 'redirect') {
      throw redirect({ to: result.to });
    }

    return { userId: result.userId };
  },
  component: ChatPage,
});

function ChatPage() {
  const { chatId } = Route.useParams();

  const agent = useAgent({
    agent: 'chat',
    name: chatId,
  });

  const chat = useAgentChat({
    agent,
    resume: true,
    // SSR: `useAgent` (PartySocket) uses a dummy host when `window` is undefined.
    // Avoid fetching `/get-messages` against that host during SSR; the client will
    // fetch messages from the real origin after hydration.
    ...(typeof window === 'undefined' ? { getInitialMessages: null } : {}),
  });

  const [input, setInput] = useState('');
  const isBusy = chat.status === 'submitted' || chat.status === 'streaming';

  const messages = chat.messages as ReadonlyArray<UIMessage>;

  const rendered = useMemo(() => {
    return messages.map((msg) => {
      const parts: ReadonlyArray<unknown> = Array.isArray((msg as any).parts) ? (msg as any).parts : [];

      const reasoning = parts
        .filter(
          (p): p is { type: 'reasoning'; text: string } =>
            Boolean(p) &&
            typeof p === 'object' &&
            (p as any).type === 'reasoning' &&
            typeof (p as any).text === 'string',
        )
        .map((p) => p.text)
        .join('');

      const text = parts
        .filter(
          (p): p is { type: 'text'; text: string } =>
            Boolean(p) &&
            typeof p === 'object' &&
            (p as any).type === 'text' &&
            typeof (p as any).text === 'string',
        )
        .map((p) => p.text)
        .join('');

      return {
        id: msg.id,
        role: msg.role,
        text,
        reasoning,
      };
    });
  }, [messages]);

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    const text = input.trim();
    if (!text || isBusy) return;

    setInput('');
    void chat.sendMessage({ text }).catch(() => {
      // Best effort: restore input if send fails.
      setInput(text);
    });
  };

  return (
    <div className="fixed inset-0">
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: whitePreset.backgroundColor,
          backgroundImage: [
            `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 15%, rgba(0,0,0,0.45) 55%, rgba(0,0,0,0.85) 100%)`,
            whitePreset.backgroundImage,
          ].join(', '),
        }}
      >
        <DotsBackground
          distance={whitePreset.distance}
          dotsColor="hsla(0, 0%, 100%, 0.03)"
          dotsSettings={{ type: 'circle', size: 2 }}
        />
      </div>

      <div className="absolute inset-0 z-10 flex min-h-full min-w-full flex-col p-4">
        <header className="mx-auto flex w-full max-w-4xl items-center justify-between py-2">
          <div className="text-sm font-semibold text-white">OpenAgents</div>
          <div className="text-xs text-white/50">Autopilot</div>
        </header>

        <main className="mx-auto flex w-full max-w-4xl flex-1 flex-col overflow-hidden rounded-lg border border-white/10 bg-black/20 backdrop-blur-sm">
          <div className="flex-1 overflow-y-auto p-4">
            <div className="flex flex-col gap-3">
              {rendered.map((m) => (
                <div
                  key={m.id}
                  className={[
                    'max-w-[90%] rounded-md px-3 py-2 text-sm leading-relaxed',
                    m.role === 'user'
                      ? 'self-end bg-white text-black'
                      : 'self-start bg-white/10 text-white',
                  ].join(' ')}
                >
                  {m.reasoning && m.role !== 'user' ? (
                    <div className="whitespace-pre-wrap text-xs text-white/60">{m.reasoning}</div>
                  ) : null}
                  {m.text ? (
                    <div className={m.reasoning && m.role !== 'user' ? 'mt-2 whitespace-pre-wrap' : 'whitespace-pre-wrap'}>
                      {m.text}
                    </div>
                  ) : (
                    <span className={m.role === 'user' ? 'text-black/40' : 'text-white/40'}>
                      (no text)
                    </span>
                  )}
                </div>
              ))}
            </div>
          </div>

          <form onSubmit={onSubmit} className="flex items-center gap-2 border-t border-white/10 p-3">
            <input
              autoFocus
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Message Autopilot…"
              disabled={isBusy}
              className="h-10 flex-1 rounded-md bg-black/40 px-3 text-sm text-white placeholder:text-white/40 outline-none ring-1 ring-white/10 focus:ring-white/20"
            />
            {isBusy ? (
              <button
                type="button"
                onClick={() => void chat.stop()}
                className="inline-flex h-10 items-center justify-center rounded-md bg-white/10 px-4 text-sm font-medium text-white hover:bg-white/20"
              >
                Stop
              </button>
            ) : (
              <button
                type="submit"
                className="inline-flex h-10 items-center justify-center rounded-md bg-white px-4 text-sm font-semibold text-black hover:bg-white/90"
              >
                Send
              </button>
            )}
          </form>
        </main>
      </div>
    </div>
  );
}
