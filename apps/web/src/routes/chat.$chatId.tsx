import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { DotsBackground, whitePreset } from '@openagentsinc/hud/react';
import { useAgent } from 'agents/react';
import { useAgentChat } from '@cloudflare/ai-chat/react';
import { useMemo, useState } from 'react';
import type { FormEvent } from 'react';

export const Route = createFileRoute('/chat/$chatId')({
  loader: async ({ params }) => {
    const { user } = await getAuth();
    if (!user) {
      throw redirect({ to: '/' });
    }

    // One Autopilot per user: the only valid chatId is their user id.
    if (params.chatId !== user.id) {
      throw redirect({ to: '/assistant' });
    }

    return { userId: user.id };
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
  });

  const [input, setInput] = useState('');
  const isBusy = chat.status === 'submitted' || chat.status === 'streaming';

  const messages = chat.messages as ReadonlyArray<unknown>;

  const rendered = useMemo(() => {
    return messages.map((m) => {
      const msg = m as any;
      const role = msg.role as string | undefined;
      const content = msg.content as unknown;

      const text =
        typeof content === 'string'
          ? content
          : Array.isArray(content)
              ? content
                  .filter((p: any) => p && typeof p === 'object' && p.type === 'text')
                  .map((p: any) => String(p.text ?? ''))
                  .join('')
              : '';

      return {
        id: msg.id ?? `${role ?? 'msg'}-${Math.random().toString(36).slice(2)}`,
        role: role ?? 'assistant',
        text,
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
                  {m.text || <span className="text-white/40">(no text)</span>}
                </div>
              ))}
            </div>
          </div>

          <form onSubmit={onSubmit} className="flex items-center gap-2 border-t border-white/10 p-3">
            <input
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
