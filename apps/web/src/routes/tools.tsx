import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { useEffect, useMemo, useState } from 'react';
import { DotsGridBackground, whitePreset } from '@openagentsinc/hud/react';
import { KranoxFrame } from '../components/KranoxFrame';
import { TelemetryService } from '../effect/telemetry';
import { AgentApiService } from '../effect/agentApi';
import type { AgentToolContract } from '../effect/agentApi';

export const Route = createFileRoute('/tools')({
  loader: async ({ context }) => {
    const result = await context.effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;

        const auth = yield* Effect.tryPromise({
          try: () => getAuth(),
          catch: (err) => err,
        }).pipe(Effect.catchAll(() => Effect.succeed(null)));

        const userId = auth?.user?.id ?? null;
        if (!userId) {
          yield* telemetry.withNamespace('route.tools').event('tools.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        yield* telemetry.withNamespace('route.tools').event('tools.open', { userId });
        return { kind: 'ok' as const, userId };
      }),
    );

    if (result.kind === 'redirect') throw redirect({ to: result.to });
    return { userId: result.userId };
  },
  component: ToolsPage,
});

function safeStableStringify(value: unknown, indent = 2): string {
  if (value == null) return String(value);
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value, null, indent);
  } catch {
    return String(value);
  }
}

function ToolsPage() {
  const { userId } = Route.useLoaderData();
  const router = useRouter();
  const runtime = router.options.context.effectRuntime;

  const [tools, setTools] = useState<ReadonlyArray<AgentToolContract> | null>(null);
  const [errorText, setErrorText] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setTools(null);
    setErrorText(null);
    runtime
      .runPromise(
        Effect.gen(function* () {
          const api = yield* AgentApiService;
          return yield* api.getToolContracts(userId);
        }),
      )
      .then((next) => {
        if (cancelled) return;
        setTools(next);
      })
      .catch((err) => {
        if (cancelled) return;
        setErrorText(err instanceof Error ? err.message : 'Failed to load tool contracts.');
      });
    return () => {
      cancelled = true;
    };
  }, [runtime, userId]);

  const sorted = useMemo(() => {
    if (!tools) return null;
    return [...tools].sort((a, b) => a.name.localeCompare(b.name));
  }, [tools]);

  return (
    <div className="fixed inset-0 overflow-hidden text-text-primary font-mono">
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: whitePreset.backgroundColor,
          backgroundImage: [
            `radial-gradient(120% 85% at 50% 0%, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0) 55%)`,
            `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0,0,0,0.55) 60%, rgba(0,0,0,0.88) 100%)`,
            whitePreset.backgroundImage,
          ].join(', '),
        }}
      >
        <DotsGridBackground
          distance={whitePreset.distance}
          dotsColor="hsla(0, 0%, 100%, 0.035)"
          lineColor="hsla(0, 0%, 100%, 0.03)"
          dotsSettings={{ type: 'circle', size: 2 }}
        />
      </div>

      <div className="relative z-10 flex flex-col h-screen overflow-hidden">
        <header className="flex items-center h-12 px-4 gap-3 border-b border-border-dark bg-bg-secondary shrink-0 shadow-[0_1px_0_rgba(255,255,255,0.06)]">
          <a
            href="/autopilot"
            className="text-accent font-mono font-bold text-base tracking-[0.12em] leading-none uppercase hover:opacity-90"
          >
            OpenAgents
          </a>
          <div className="h-6 w-px bg-border-dark/70" aria-hidden="true" />
          <span className="text-xs text-text-dim uppercase tracking-wider">DSE Tools</span>
        </header>

        <main className="flex-1 min-h-0 w-full p-4 overflow-hidden">
          <div className="mx-auto w-full max-w-5xl h-full min-h-0">
            <KranoxFrame className="h-full min-h-0">
              <div className="flex h-full min-h-0 flex-col px-4 py-4 sm:px-6 lg:px-8">
                <div className="flex items-baseline justify-between gap-4">
                  <div>
                    <div className="text-xs text-text-dim uppercase tracking-wider">
                      Tool Contracts
                    </div>
                    <div className="text-[11px] text-text-muted mt-1">
                      Source: Autopilot Worker `GET /agents/chat/:id/tool-contracts`
                    </div>
                  </div>
                  <a
                    href="/signatures"
                    className="text-[11px] text-text-muted hover:text-text-primary"
                  >
                    View signatures →
                  </a>
                </div>

                <div className="mt-4 flex-1 min-h-0 overflow-y-auto overseer-scroll pr-1">
                  {errorText ? (
                    <div className="text-xs text-red-400">Error: {errorText}</div>
                  ) : !sorted ? (
                    <div className="text-xs text-text-dim">Loading…</div>
                  ) : sorted.length === 0 ? (
                    <div className="text-xs text-text-dim">(no tools)</div>
                  ) : (
                    <div className="flex flex-col gap-3">
                      {sorted.map((t) => {
                        const inputSchema = safeStableStringify(t.inputSchemaJson);
                        const outputSchema = safeStableStringify(t.outputSchemaJson);
                        return (
                          <details
                            key={t.name}
                            className="rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset]"
                          >
                            <summary className="cursor-pointer select-none px-3 py-2 flex items-baseline gap-3">
                              <span className="text-xs font-semibold text-text-primary">
                                {t.name}
                              </span>
                              {t.usage ? (
                                <span className="text-[10px] font-mono text-text-dim break-words">
                                  {t.usage}
                                </span>
                              ) : null}
                            </summary>
                            <div className="border-t border-border-dark/70 px-3 py-2">
                              <div className="text-[11px] text-text-muted whitespace-pre-wrap break-words">
                                {t.description}
                              </div>

                              <div className="mt-3">
                                <div className="text-[10px] text-text-dim uppercase tracking-wider mb-1">
                                  Input Schema (JSON Schema)
                                </div>
                                <pre className="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">
                                  {inputSchema}
                                </pre>
                              </div>

                              <div className="mt-3 border-t border-border-dark/60 border-dashed pt-2">
                                <div className="text-[10px] text-text-dim uppercase tracking-wider mb-1">
                                  Output Schema (JSON Schema)
                                </div>
                                <pre className="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">
                                  {outputSchema}
                                </pre>
                              </div>
                            </div>
                          </details>
                        );
                      })}
                    </div>
                  )}
                </div>
              </div>
            </KranoxFrame>
          </div>
        </main>
      </div>
    </div>
  );
}
