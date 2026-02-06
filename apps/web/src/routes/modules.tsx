import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { whitePreset } from '@openagentsinc/hud';
import { EffuseMount } from '../components/EffuseMount';
import { cleanupHudBackground, runHudDotsGridBackground } from '../effuse-pages/hudBackground';
import { runModulesPage } from '../effuse-pages/modules';
import { TelemetryService } from '../effect/telemetry';
import { AgentRpcClientService } from '../effect/api/agentRpcClient';
import { AgentApiService } from '../effect/agentApi';
import type { DseModuleContract } from '../effect/agentApi';
import type { ModuleItem } from '../effuse-pages/modules';

export const Route = createFileRoute('/modules')({
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
          yield* telemetry.withNamespace('route.modules').event('modules.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        yield* telemetry.withNamespace('route.modules').event('modules.open', { userId });
        return { kind: 'ok' as const, userId };
      }),
    );

    if (result.kind === 'redirect') throw redirect({ to: result.to });
    return { userId: result.userId };
  },
  component: ModulesPage,
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

function ModulesPage() {
  const { userId } = Route.useLoaderData();
  const router = useRouter();
  const runtime = router.options.context.effectRuntime;

  const [mods, setMods] = useState<ReadonlyArray<DseModuleContract> | null>(null);
  const [errorText, setErrorText] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setMods(null);
    setErrorText(null);
    runtime
      .runPromise(
        Effect.gen(function* () {
          const rpc = yield* AgentRpcClientService;
          return yield* rpc.agent.getModuleContracts({ chatId: userId });
        }).pipe(
          // Keep the legacy HTTP path as a fallback while RPC is being proven out.
          Effect.catchAll(() =>
            Effect.gen(function* () {
              const api = yield* AgentApiService;
              return yield* api.getModuleContracts(userId);
            }),
          ),
        ),
      )
      .then((next) => {
        if (cancelled) return;
        setMods(next);
      })
      .catch((err) => {
        if (cancelled) return;
        setErrorText(err instanceof Error ? err.message : 'Failed to load module contracts.');
      });
    return () => {
      cancelled = true;
    };
  }, [runtime, userId]);

  const pageData = useMemo((): { errorText: string | null; sorted: ReadonlyArray<ModuleItem> | null } => {
    if (errorText) return { errorText, sorted: null };
    if (!mods) return { errorText: null, sorted: null };
    const sorted = [...mods].sort((a, b) => a.moduleId.localeCompare(b.moduleId));
    return {
      errorText: null,
      sorted: sorted.map((m) => ({
        moduleId: m.moduleId,
        description: m.description,
        signatureIdsJson: safeStableStringify(m.signatureIds),
      })),
    };
  }, [mods, errorText]);

  const run = useCallback(
    (el: Element) => runModulesPage(el, pageData),
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
        <EffuseMount
          run={(el) =>
            runHudDotsGridBackground(el, {
              distance: whitePreset.distance,
              dotsColor: 'hsla(0, 0%, 100%, 0.035)',
              lineColor: 'hsla(0, 0%, 100%, 0.03)',
              dotsSettings: { type: 'circle', size: 2 },
            })
          }
          onCleanup={cleanupHudBackground}
          className="absolute inset-0 pointer-events-none"
        />
      </div>
      <EffuseMount run={run} deps={[pageData]} className="relative z-10 flex flex-col h-screen overflow-hidden" />
    </div>
  );
}
