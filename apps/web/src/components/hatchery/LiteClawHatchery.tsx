import { useCallback, useEffect, useState } from 'react';
import { Link, useNavigate } from '@tanstack/react-router';
import { useAgent } from 'agents/react';
import { useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { MessageType } from '@cloudflare/ai-chat/types';
import { Animator, useAnimator } from '@arwes/react-animator';
import { ANIMATOR_ACTIONS } from '@arwes/animator';
import { Puffs } from '@arwes/react-bgs';
import { DotsGridBackground, purplePreset } from '@openagentsinc/hud/react';
import { AssemblingFrame } from './AssemblingFrame';
import { api } from '../../../convex/_generated/api';
import { posthogCapture } from '@/lib/posthog';
import { HatcheryButton } from './HatcheryButton';
import { HatcheryH1, HatcheryH2, HatcheryP } from './HatcheryTypography';
import { Input } from '@/components/ui/input';
import { MessageSquareIcon, ServerIcon } from 'lucide-react';

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function FrameAnimatorController({ active }: { active: boolean }) {
  const animator = useAnimator();

  useEffect(() => {
    if (!animator) {
      return;
    }
    animator.node.send(active ? ANIMATOR_ACTIONS.enter : ANIMATOR_ACTIONS.exit);
  }, [animator, active]);

  return null;
}

export function LiteClawHatchery() {
  const { user, loading: authLoading } = useAuth();
  const accessStatus = useQuery(api.access.getStatus);
  const liteclawThreadId = useQuery(api.threads.getLiteclawThread);
  const getOrCreateLiteclawThread = useMutation(
    api.threads.getOrCreateLiteclawThread,
  );
  const [email, setEmail] = useState('');
  const [waitlistStatus, setWaitlistStatus] = useState<'idle' | 'submitting' | 'success' | 'error'>('idle');
  const [waitlistError, setWaitlistError] = useState<string | null>(null);
  const joinWaitlistMutation = useMutation(api.waitlist.joinWaitlist);
  const [spawnStatus, setSpawnStatus] = useState<'idle' | 'spawning' | 'ready' | 'error'>('idle');
  const [spawnError, setSpawnError] = useState<string | null>(null);
  const [resetStatus, setResetStatus] = useState<'idle' | 'resetting' | 'success' | 'error'>('idle');
  const [resetError, setResetError] = useState<string | null>(null);
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
  const hasLiteclaw = liteclawThreadId != null;
  const canConnectAgent = accessAllowed && Boolean(liteclawThreadId);

  const agent = useAgent({
    agent: 'chat',
    name: liteclawThreadId ?? 'pending',
    startClosed: !canConnectAgent,
  });

  const sendClearCommand = useCallback(async () => {
    if (agent.readyState === WebSocket.OPEN) {
      agent.send(JSON.stringify({ type: MessageType.CF_AGENT_CHAT_CLEAR }));
      return;
    }

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        cleanup();
        reject(new Error('Timed out waiting for LiteClaw connection.'));
      }, 8000);

      const cleanup = () => {
        clearTimeout(timeout);
        agent.removeEventListener('open', handleOpen);
        agent.removeEventListener('error', handleError);
      };

      const handleOpen = () => {
        cleanup();
        agent.send(JSON.stringify({ type: MessageType.CF_AGENT_CHAT_CLEAR }));
        resolve();
      };

      const handleError = () => {
        cleanup();
        reject(new Error('Failed to connect to LiteClaw.'));
      };

      agent.addEventListener('open', handleOpen);
      agent.addEventListener('error', handleError);
      agent.reconnect();
    });
  }, [agent]);

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
    if (hasLiteclaw && liteclawThreadId) {
      navigate({ to: '/chat/$chatId', params: { chatId: liteclawThreadId } });
      return;
    }
    setSpawnStatus('spawning');
    setSpawnError(null);
    try {
      const threadId = await getOrCreateLiteclawThread({});
      setSpawnStatus('ready');
      posthogCapture('hatchery_liteclaw_spawn');
      navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
    } catch (error) {
      setSpawnError(error instanceof Error ? error.message : 'Failed to spawn LiteClaw');
      setSpawnStatus('error');
    }
  };

  const handleReset = async () => {
    if (!canConnectAgent || !liteclawThreadId) {
      setResetError('LiteClaw is not ready yet.');
      setResetStatus('error');
      return;
    }
    setResetStatus('resetting');
    setResetError(null);
    try {
      await sendClearCommand();
      setResetStatus('success');
      posthogCapture('hatchery_liteclaw_reset');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to reset LiteClaw';
      setResetError(message);
      setResetStatus('error');
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
    <Animator
      root
      active={frameVisible}
      combine
      duration={{ enter: 0.8, exit: 0.3, interval: 4 }}
    >
      <FrameAnimatorController active={frameVisible} />
      <div
        className="min-h-screen bg-site flex flex-col p-3 md:p-4"
        style={{ fontFamily: 'var(--font-square721)' }}
      >
        <nav className="relative z-20 mb-3 md:mb-4">
          <div className="mx-auto flex max-w-6xl items-center justify-between px-4 py-2">
            <div className="flex items-center gap-2">
              <span className="text-xl font-bold text-foreground">Hatchery</span>
            </div>
          </div>
        </nav>
        <div className="relative flex min-h-0 flex-1 flex-col">
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
          <Puffs
            color="hsla(280, 50%, 70%, 0.2)"
            quantity={20}
          />
        </div>
        <div className="relative z-10 flex flex-1 flex-col p-4">
          <div className="mb-4 flex justify-end">
            <HatcheryButton
              variant="outline"
              onClick={() =>
                setFrameVisible((visible) => {
                  const next = !visible;
                  console.log('[hatchery][button] toggle frameVisible', {
                    from: visible,
                    to: next,
                  });
                  return next;
                })
              }
            >
              {frameVisible ? 'Hide frame & puffs' : 'Show frame & puffs'}
            </HatcheryButton>
          </div>
          <AssemblingFrame
            className="mx-auto w-full max-w-2xl"
            onReady={handleFrameReady}
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
                        {liteclawThreadId === undefined
                          ? '…'
                          : hasLiteclaw
                            ? 'ready'
                            : spawnStatus === 'spawning'
                              ? 'spawning'
                              : spawnStatus === 'error'
                                ? 'error'
                                : 'not spawned'}
                      </HatcheryP>
                    </div>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <HatcheryButton
                      onClick={handleSpawn}
                      disabled={spawnStatus === 'spawning'}
                    >
                      {hasLiteclaw ? (
                        <>
                          <MessageSquareIcon className="mr-2 size-4" />
                          Go to chat
                        </>
                      ) : spawnStatus === 'spawning' ? (
                        'Spawning…'
                      ) : (
                        'Spawn your LiteClaw'
                      )}
                    </HatcheryButton>
                    <HatcheryButton
                      variant="outline"
                      onClick={handleReset}
                      disabled={!hasLiteclaw || resetStatus === 'resetting'}
                    >
                      {resetStatus === 'resetting' ? 'Resetting…' : 'Reset LiteClaw memory'}
                    </HatcheryButton>
                  </div>
                  {spawnError && (
                    <HatcheryP className="text-destructive">{spawnError}</HatcheryP>
                  )}
                  {resetError && (
                    <HatcheryP className="text-destructive">{resetError}</HatcheryP>
                  )}
                  {resetStatus === 'success' && !resetError && (
                    <HatcheryP>LiteClaw memory cleared.</HatcheryP>
                  )}
                  {hasLiteclaw && liteclawThreadId && (
                    <Link
                      to="/chat/$chatId"
                      params={{ chatId: liteclawThreadId }}
                      className="text-muted-foreground hover:text-foreground text-sm underline"
                    >
                      Open chat →
                    </Link>
                  )}
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
    </Animator>
  );
}
