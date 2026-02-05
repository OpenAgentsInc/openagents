import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from '@tanstack/react-router';
import { useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { DotsGridBackground, whitePreset } from '@openagentsinc/hud/react';
import { AssemblingFrame } from './AssemblingFrame';
import { api } from '../../../convex/_generated/api';
import { posthogCapture } from '@/lib/posthog';
import { HatcheryButton } from './HatcheryButton';
import { HatcheryH1, HatcheryH2, HatcheryP } from './HatcheryTypography';
import { HatcheryPuffs } from './HatcheryPuffs';
import { Input } from '@/components/ui/input';
import { ArrowLeft, ServerIcon } from 'lucide-react';

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function AutopilotPage() {
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
  const autopilotThreads = useMemo(() => {
    const items = (threads ?? []).filter(
      (thread) => thread.kind === 'autopilot' || thread.kind === 'liteclaw',
    );
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
    posthogCapture('autopilot_waitlist_submit', { source: 'autopilot' });
    try {
      await joinWaitlistMutation({ email: trimmed, source: 'autopilot' });
      posthogCapture('autopilot_waitlist_success', { source: 'autopilot' });
      setWaitlistStatus('success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to join waitlist';
      setWaitlistError(message);
      setWaitlistStatus('error');
      posthogCapture('autopilot_waitlist_error', { source: 'autopilot', message });
    }
  };

  const handleSpawn = async () => {
    setSpawnStatus('spawning');
    setSpawnError(null);
    try {
      const title =
        autopilotThreads.length > 0 ? `Autopilot ${autopilotThreads.length + 1}` : 'Autopilot';
      const threadId = await createThread({ title, kind: 'autopilot' });
      setSpawnStatus('ready');
      posthogCapture('autopilot_spawn', { kind: 'new' });
      navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
    } catch (error) {
      setSpawnError(error instanceof Error ? error.message : 'Failed to spawn Autopilot');
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
            <HatcheryButton
              type="button"
              variant="outline"
              className="text-sm"
              onClick={() => navigate({ to: '/' })}
            >
              <ArrowLeft className="size-4 shrink-0" aria-hidden />
              Back to OpenAgents
            </HatcheryButton>
          </div>
        </div>
      </nav>
      <div className="relative flex min-h-0 flex-1 flex-col pt-20">
        {/* Arwes-style dots + grid background (white preset) + vignette + puffs */}
        <div
          className="absolute inset-0"
          style={{
            backgroundColor: whitePreset.backgroundColor,
            backgroundImage: [
              `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 15%, rgba(0,0,0,0.4) 55%, rgba(0,0,0,0.75) 100%)`,
              whitePreset.backgroundImage,
            ].join(', '),
          }}
        >
          <DotsGridBackground
            distance={whitePreset.distance}
            dotsColor={whitePreset.dotsColor}
            lineColor={whitePreset.lineColor}
          />
          <HatcheryPuffs />
        </div>
        <div className="relative z-10 flex flex-1 flex-col p-4">
          <AssemblingFrame
            className="mx-auto w-full max-w-2xl"
            onReady={handleFrameReady}
            active={frameVisible}
          >
            {/* Waitlist overlay */}
            {overlayVisible && (
              <div className="flex min-h-[50vh] flex-col items-center justify-center gap-6 text-center">
          <HatcheryH1>Autopilot Early Access</HatcheryH1>
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

            {/* Autopilot content when access allowed */}
            {accessAllowed && (
              <div className="flex flex-col gap-6">
                <div>
                  <HatcheryH1>Autopilot</HatcheryH1>
                  <HatcheryP className="mt-1">
                    Spawn your Autopilot — a persistent chat agent that remembers context.
                  </HatcheryP>
                </div>
	                <div className="flex flex-col gap-4">
	                  <div className="flex items-center gap-3">
	                    <ServerIcon className="size-8 text-muted-foreground" />
	                    <div>
	                      <HatcheryH2>Autopilot</HatcheryH2>
	                      <HatcheryP>
	                        Status:{' '}
	                        {threads === undefined
	                          ? '…'
	                          : spawnStatus === 'spawning'
	                            ? 'spawning'
	                            : spawnStatus === 'error'
	                              ? 'error'
	                              : autopilotThreads.length
	                                ? `${autopilotThreads.length} active`
	                                : 'none yet'}
	                      </HatcheryP>
	                    </div>
	                  </div>

	                  <div className="flex flex-col gap-3 rounded-2xl border border-white/10 bg-black/20 p-4">
	                    <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
	                      <div>
	                        <HatcheryH2 durationMs={450}>Your Autopilots</HatcheryH2>
	                        <HatcheryP className="mt-1">
	                          Each Autopilot is its own persistent agent with separate memory.
	                        </HatcheryP>
	                      </div>
	                      <HatcheryButton
	                        onClick={handleSpawn}
	                        disabled={spawnStatus === 'spawning'}
	                      >
	                        {spawnStatus === 'spawning'
	                          ? 'Spawning…'
	                          : autopilotThreads.length
	                            ? 'Spawn another Autopilot'
	                            : 'Spawn your first Autopilot'}
	                      </HatcheryButton>
	                    </div>

	                    {threads === undefined ? (
	                      <HatcheryP>Loading…</HatcheryP>
	                    ) : autopilotThreads.length ? (
	                      <div className="flex flex-col gap-2">
	                        {autopilotThreads.map((thread) => (
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
	                      <HatcheryP>No Autopilots yet. Spawn one to start.</HatcheryP>
	                    )}
	                  </div>

	                  {spawnError && (
	                    <HatcheryP className="text-destructive">{spawnError}</HatcheryP>
	                  )}

	                  {autopilotThreads.length ? (
	                    <Link
	                      to="/chat/$chatId"
	                      params={{ chatId: autopilotThreads[0]!._id }}
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
                Sign in to spawn your Autopilot.
              </HatcheryP>
            )}
          </AssemblingFrame>
        </div>
      </div>
    </div>
  );
}
