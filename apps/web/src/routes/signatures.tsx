import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { DotsGridBackground, whitePreset } from '@openagentsinc/hud/react';
import { EffuseMount } from '../components/EffuseMount';
import { runSignaturesPage } from '../effuse-pages/signatures';
import { TelemetryService } from '../effect/telemetry';
import { AgentApiService } from '../effect/agentApi';
import type { DseSignatureContract } from '../effect/agentApi';

export const Route = createFileRoute('/signatures')({
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
          yield* telemetry.withNamespace('route.signatures').event('signatures.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        yield* telemetry.withNamespace('route.signatures').event('signatures.open', { userId });
        return { kind: 'ok' as const, userId };
      }),
    );

    if (result.kind === 'redirect') throw redirect({ to: result.to });
    return { userId: result.userId };
  },
  component: SignaturesPage,
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

function summarizePromptIr(promptIr: unknown): string {
  if (!promptIr || typeof promptIr !== 'object') return '(missing prompt IR)';
  const obj = promptIr as { blocks?: unknown[] };
  const blocks = obj.blocks;
  if (!Array.isArray(blocks)) return '(missing blocks)';
  const tags = blocks.map((b) => (b && typeof b === 'object' ? String((b as { _tag?: unknown })._tag ?? '?') : '?'));
  return tags.join(' → ');
}

function SignaturesPage() {
  const { userId } = Route.useLoaderData();
  const router = useRouter();
  const runtime = router.options.context.effectRuntime;

  const [sigs, setSigs] = useState<ReadonlyArray<DseSignatureContract> | null>(null);
  const [errorText, setErrorText] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSigs(null);
    setErrorText(null);
    runtime
      .runPromise(
        Effect.gen(function* () {
          const api = yield* AgentApiService;
          return yield* api.getSignatureContracts(userId);
        }),
      )
      .then((next) => {
        if (cancelled) return;
        setSigs(next);
      })
      .catch((err) => {
        if (cancelled) return;
        setErrorText(err instanceof Error ? err.message : 'Failed to load signature contracts.');
      });
    return () => {
      cancelled = true;
    };
  }, [runtime, userId]);

  const pageData = useMemo((): {
    errorText: string | null;
    sorted: ReadonlyArray<import('../effuse-pages/signatures').SignatureItem> | null;
  } => {
    if (errorText) return { errorText, sorted: null };
    if (!sigs) return { errorText: null, sorted: null };
    const sorted = [...sigs].sort((a, b) => a.signatureId.localeCompare(b.signatureId));
    return {
      errorText: null,
      sorted: sorted.map((s) => ({
        signatureId: s.signatureId,
        promptSummary: summarizePromptIr(s.promptIr),
        inputSchemaJson: safeStableStringify(s.inputSchemaJson),
        outputSchemaJson: safeStableStringify(s.outputSchemaJson),
        promptIrJson: safeStableStringify(s.promptIr),
        defaultsJson: safeStableStringify({
          defaultParams: s.defaultParams,
          defaultConstraints: s.defaultConstraints,
        }),
      })),
    };
  }, [sigs, errorText]);

  const run = useCallback(
    (el: Element) => runSignaturesPage(el, pageData),
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
