import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from '@tanstack/react-router';
import { useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { DotsGridBackground, purplePreset } from '@openagentsinc/hud/react';
import { AssemblingFrame } from './AssemblingFrame';
import { api } from '../../../convex/_generated/api';
import { posthogCapture } from '@/lib/posthog';
import { HatcheryButton } from './HatcheryButton';
import { HatcheryH1, HatcheryH2, HatcheryP } from './HatcheryTypography';
import { HatcheryPuffs } from './HatcheryPuffs';
import { Input } from '@/components/ui/input';
import { ServerIcon } from 'lucide-react';

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function LiteClawHatchery() {
  const { user, loading: authLoading } = useAuth();
  const accessStatus = useQuery(api.access.getStatus);
  const threads = useQuery(api.threads.list, { archived: false, limit: 200 });
  const createThread = useMutation(api.threads.create);
  const [email, setEmail] = useState('');
  const [waitlistStatus, setWaitlistStatus] = useState<'idle' | 'submitting' | 'success' | 'error'>('idle');
  const [waitlistError, setWaitlistError] = useState<string | null>(null);
  const joinWaitlistMutation = useMutation(api.waitlist.joinWaitlist);
  const [spawnStatus, setSpawnStatus] = useState<'idle' | 'spawning' | 'ready' | 'error'>('idle');
  const [spawnError, setSpawnError] = useState<string | null>(null);
  const [frameVisible, setFrameVisible] = useState(false);
  const [frameReady, setFrameReady] = useState(false);
  const navigate = useNavigate();
  const handleFrameReady = useCallback(() => {
    setFrameReady(true);
  }, []);

  useEffect(() => {
    if (!frameReady) {
      return;
    }
    const id = window.setTimeout(() => setFrameVisible(true), 500);
    return () => window.clearTimeout(id);
  }, [frameReady]);

  const accessAllowed = accessStatus?.allowed === true;
  const waitlistEntry = accessStatus?.waitlistEntry ?? null;
  const waitlistApproved = accessStatus?.waitlistApproved === true;
  const overlayVisible = !accessAllowed;
  const liteclaws = useMemo(() => {
    const items = (threads ?? []).filter((thread) => thread.kind === 'liteclaw');
    items.sort((a, b) => b.updated_at - a.updated_at);
    return items;
  }, [threads]);

  const handleJoinWaitlist = async () => {
    const trimmed = email.trim();
    if (!EMAIL_RE.test(trimmed)) {
      setWaitlistError('Please enter a valid email.');
      return;
    }
    setWaitlistStatus('submitting');
    setWaitlistError(null);
    posthogCapture('hatchery_waitlist_submit', { source: 'hatchery' });
    try {
      await joinWaitlistMutation({ email: trimmed, source: 'hatchery' });
      posthogCapture('hatchery_waitlist_success', { source: 'hatchery' });
      setWaitlistStatus('success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to join waitlist';
      setWaitlistError(message);
      setWaitlistStatus('error');
      posthogCapture('hatchery_waitlist_error', { source: 'hatchery', message });
    }
  };

  const handleSpawn = async () => {
    setSpawnStatus('spawning');
    setSpawnError(null);
    try {
      const title =
        liteclaws.length > 0 ? `LiteClaw ${liteclaws.length + 1}` : 'LiteClaw';
      const threadId = await createThread({ title, kind: 'liteclaw' });
      setSpawnStatus('ready');
      posthogCapture('hatchery_liteclaw_spawn', { kind: 'new' });
      navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
    } catch (error) {
      setSpawnError(error instanceof Error ? error.message : 'Failed to spawn LiteClaw');
      setSpawnStatus('error');
    }
  };

  if (authLoading || accessStatus === undefined) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center p-4">
        <span className="text-muted-foreground text-sm">Checking access…</span>
      </div>
    );
  }

  return (
    <div
      className="min-h-screen bg-site flex flex-col p-3 md:p-4"
      style={{ fontFamily: 'var(--font-square721)' }}
    >
      <nav className="absolute left-0 right-0 top-0 z-20 select-none pt-4 md:pt-5">
        <div className="mx-auto flex max-w-6xl items-center justify-between px-4 py-2">
          <div className="flex items-center gap-2">
            <span className="text-xl font-bold text-foreground">Hatchery</span>
          </div>
        </div>
      </nav>
      <div className="relative flex min-h-0 flex-1 flex-col pt-20">
        {/* Arwes-style dots + grid background (purple preset) + vignette + puffs */}
        <div
          className="absolute inset-0"
          style={{
            backgroundColor: purplePreset.backgroundColor,
            backgroundImage: [
              `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 15%, rgba(0,0,0,0.4) 55%, rgba(0,0,0,0.75) 100%)`,
              purplePreset.backgroundImage,
            ].join(', '),
          }}
        >
          <DotsGridBackground
            distance={purplePreset.distance}
            dotsColor={purplePreset.dotsColor}
            lineColor={purplePreset.lineColor}
          />
          <HatcheryPuffs />
        </div>
        <div className="relative z-10 flex flex-1 flex-col p-4">
          {/* <div className="mb-4 flex justify-end">
            <HatcheryButton
              variant="outline"
              onClick={() => setFrameVisible((visible) => !visible)}
            >
              {frameVisible ? 'Hide frame & puffs' : 'Show frame & puffs'}
            </HatcheryButton>
          </div> */}
          <AssemblingFrame
            className="mx-auto w-full max-w-2xl"
            onReady={handleFrameReady}
            active={frameVisible}
          >
            {/* Waitlist overlay */}
            {overlayVisible && (
              <div className="flex min-h-[50vh] flex-col items-center justify-center gap-6 text-center">
          <HatcheryH1>LiteClaw Early Access</HatcheryH1>
          <HatcheryP className="max-w-md">
            A persistent, personal AI agent that remembers context and feels always there — no setup friction.
          </HatcheryP>
          {accessStatus === null ? (
            <HatcheryP>Checking access…</HatcheryP>
          ) : waitlistApproved || waitlistStatus === 'success' || waitlistEntry ? (
            <div className="flex flex-col items-center gap-3">
              <HatcheryP>Thanks! We&apos;ll email you as soon as access opens.</HatcheryP>
            </div>
          ) : (
            <div className="flex w-full max-w-sm flex-col gap-3">
              <HatcheryP>Join the waitlist to get access.</HatcheryP>
              <Input
                type="email"
                placeholder="you@example.com"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleJoinWaitlist()}
                disabled={waitlistStatus === 'submitting'}
                className="w-full"
              />
              <HatcheryButton
                onClick={handleJoinWaitlist}
                disabled={waitlistStatus === 'submitting'}
              >
                {waitlistStatus === 'submitting' ? 'Joining…' : 'Join the waitlist'}
              </HatcheryButton>
              {waitlistError && (
                <p className="text-destructive text-sm">{waitlistError}</p>
              )}
            </div>
          )}
            </div>
            )}

            {/* Hatchery content when access allowed */}
            {accessAllowed && (
              <div className="flex flex-col gap-6">
                <div>
                  <HatcheryH1>Hatchery</HatcheryH1>
                  <HatcheryP className="mt-1">
                    Spawn your LiteClaw — a persistent chat agent that remembers context.
                  </HatcheryP>
	                </div>
	                <div className="flex flex-col gap-4">
	                  <div className="flex items-center gap-3">
	                    <ServerIcon className="size-8 text-muted-foreground" />
	                    <div>
	                      <HatcheryH2>LiteClaw</HatcheryH2>
	                      <HatcheryP>
	                        Status:{' '}
	                        {threads === undefined
	                          ? '…'
	                          : spawnStatus === 'spawning'
	                            ? 'spawning'
	                            : spawnStatus === 'error'
	                              ? 'error'
	                              : liteclaws.length
	                                ? `${liteclaws.length} active`
	                                : 'none yet'}
	                      </HatcheryP>
	                    </div>
	                  </div>

	                  <div className="flex flex-col gap-3 rounded-2xl border border-white/10 bg-black/20 p-4">
	                    <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
	                      <div>
	                        <HatcheryH2 durationMs={450}>Your LiteClaws</HatcheryH2>
	                        <HatcheryP className="mt-1">
	                          Each LiteClaw is its own persistent agent with separate memory.
	                        </HatcheryP>
	                      </div>
	                      <HatcheryButton
	                        onClick={handleSpawn}
	                        disabled={spawnStatus === 'spawning'}
	                      >
	                        {spawnStatus === 'spawning'
	                          ? 'Spawning…'
	                          : liteclaws.length
	                            ? 'Spawn another LiteClaw'
	                            : 'Spawn your first LiteClaw'}
	                      </HatcheryButton>
	                    </div>

	                    {threads === undefined ? (
	                      <HatcheryP>Loading…</HatcheryP>
	                    ) : liteclaws.length ? (
	                      <div className="flex flex-col gap-2">
	                        {liteclaws.map((thread) => (
	                          <div
	                            key={thread._id}
	                            className="flex items-center justify-between gap-3 rounded-xl border border-white/10 bg-black/15 px-3 py-2"
	                          >
	                            <div className="min-w-0">
	                              <div className="truncate text-sm font-medium text-foreground">
	                                {thread.title}
	                              </div>
	                              <div className="text-xs text-muted-foreground">
	                                Updated{' '}
	                                {new Date(thread.updated_at).toLocaleString()}
	                              </div>
	                            </div>
	                            <HatcheryButton
	                              variant="outline"
	                              onClick={() =>
	                                navigate({
	                                  to: '/chat/$chatId',
	                                  params: { chatId: thread._id },
	                                })
	                              }
	                            >
	                              Open chat
	                            </HatcheryButton>
	                          </div>
	                        ))}
	                      </div>
	                    ) : (
	                      <HatcheryP>No LiteClaws yet. Spawn one to start.</HatcheryP>
	                    )}
	                  </div>

	                  {spawnError && (
	                    <HatcheryP className="text-destructive">{spawnError}</HatcheryP>
	                  )}

	                  {/* Reset LiteClaw memory is intentionally disabled for now.
	                      (We may reintroduce this once “memory” semantics are clearer:
	                      transcript vs summary vs tool-state.) */}
	                  {/*
	                  <HatcheryButton variant="outline" disabled>
	                    Reset LiteClaw memory
	                  </HatcheryButton>
	                  */}

	                  {liteclaws.length ? (
	                    <Link
	                      to="/chat/$chatId"
	                      params={{ chatId: liteclaws[0]!._id }}
	                      className="text-muted-foreground hover:text-foreground text-sm underline"
	                    >
	                      Open most recent chat →
	                    </Link>
	                  ) : null}
	                </div>
	              </div>
	            )}

            {!user && accessStatus !== undefined && !overlayVisible && (
              <HatcheryP className="text-center">
                Sign in to spawn your LiteClaw.
              </HatcheryP>
            )}
          </AssemblingFrame>
        </div>
      </div>
    </div>
  );
}
