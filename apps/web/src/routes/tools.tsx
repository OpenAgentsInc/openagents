import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { DotsGridBackground, whitePreset } from '@openagentsinc/hud/react';
import { EffuseMount } from '../components/EffuseMount';
import { runToolsPage } from '../effuse-pages/tools';
import { TelemetryService } from '../effect/telemetry';
import { AgentApiService } from '../effect/agentApi';
import type { AgentToolContract } from '../effect/agentApi';
import type { ToolItem } from '../effuse-pages/tools';

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

  const pageData = useMemo((): { errorText: string | null; sorted: ReadonlyArray<ToolItem> | null } => {
    if (errorText) return { errorText, sorted: null };
    if (!tools) return { errorText: null, sorted: null };
    const sorted = [...tools].sort((a, b) => a.name.localeCompare(b.name));
    return {
      errorText: null,
      sorted: sorted.map((t) => ({
        name: t.name,
        description: t.description,
        usage: t.usage ?? null,
        inputSchemaJson: safeStableStringify(t.inputSchemaJson),
        outputSchemaJson: safeStableStringify(t.outputSchemaJson ?? null),
      })),
    };
  }, [tools, errorText]);

  const run = useCallback(
    (el: Element) => runToolsPage(el, pageData),
    [pageData],
  );

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
      <EffuseMount run={run} deps={[pageData]} className="relative z-10 flex flex-col h-screen overflow-hidden" />
    </div>
  );
}
