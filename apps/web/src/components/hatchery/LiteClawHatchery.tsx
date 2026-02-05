import { useCallback, useState } from 'react';
import { Link, useNavigate } from '@tanstack/react-router';
import { useAgent } from 'agents/react';
import { useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { MessageType } from '@cloudflare/ai-chat/types';
import { Animator } from '@arwes/react-animator';
import { DotsGridBackground, purplePreset } from '@openagentsinc/hud/react';
import { AssemblingFrame } from './AssemblingFrame';
import { api } from '../../../convex/_generated/api';
import { posthogCapture } from '@/lib/posthog';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { MessageSquareIcon, ServerIcon } from 'lucide-react';

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

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
  const navigate = useNavigate();

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
    <div className="relative flex min-h-0 flex-1 flex-col">
      {/* Arwes-style dots + grid background (purple preset) */}
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: purplePreset.backgroundColor,
          backgroundImage: purplePreset.backgroundImage,
        }}
      >
        <DotsGridBackground
          distance={purplePreset.distance}
          dotsColor={purplePreset.dotsColor}
          lineColor={purplePreset.lineColor}
        />
      </div>
      <div className="relative z-10 flex flex-1 flex-col p-4">
        <Animator duration={{ enter: 0.8 }}>
          <AssemblingFrame className="mx-auto w-full max-w-2xl">
            {/* Waitlist overlay */}
            {overlayVisible && (
              <div className="flex min-h-[50vh] flex-col items-center justify-center gap-6 text-center">
          <h1 className="font-semibold text-2xl">LiteClaw Early Access</h1>
          <p className="text-muted-foreground max-w-md text-sm">
            A persistent, personal AI agent that remembers context and feels always there — no setup friction.
          </p>
          {accessStatus === null ? (
            <span className="text-muted-foreground text-sm">Checking access…</span>
          ) : waitlistApproved || waitlistStatus === 'success' || waitlistEntry ? (
            <div className="flex flex-col items-center gap-3">
              <p className="text-muted-foreground text-sm">
                Thanks! We&apos;ll email you as soon as access opens.
              </p>
            </div>
          ) : (
            <div className="flex w-full max-w-sm flex-col gap-3">
              <p className="text-muted-foreground text-sm">
                Join the waitlist to get access.
              </p>
              <Input
                type="email"
                placeholder="you@example.com"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleJoinWaitlist()}
                disabled={waitlistStatus === 'submitting'}
                className="w-full"
              />
              <Button
                onClick={handleJoinWaitlist}
                disabled={waitlistStatus === 'submitting'}
              >
                {waitlistStatus === 'submitting' ? 'Joining…' : 'Join the waitlist'}
              </Button>
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
            <h1 className="font-semibold text-2xl">Hatchery</h1>
            <p className="text-muted-foreground mt-1 text-sm">
              Spawn your LiteClaw — a persistent chat agent that remembers context.
            </p>
          </div>

          <Card>
            <CardContent className="pt-6">
              <div className="flex flex-col gap-4">
                <div className="flex items-center gap-3">
                  <ServerIcon className="size-8 text-muted-foreground" />
                  <div>
                    <p className="font-medium">LiteClaw</p>
                    <p className="text-muted-foreground text-sm">
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
                    </p>
                  </div>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button
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
                  </Button>
                  <Button
                    variant="outline"
                    onClick={handleReset}
                    disabled={!hasLiteclaw || resetStatus === 'resetting'}
                  >
                    {resetStatus === 'resetting' ? 'Resetting…' : 'Reset LiteClaw memory'}
                  </Button>
                </div>
                {spawnError && (
                  <p className="text-destructive text-sm">{spawnError}</p>
                )}
                {resetError && (
                  <p className="text-destructive text-sm">{resetError}</p>
                )}
                {resetStatus === 'success' && !resetError && (
                  <p className="text-muted-foreground text-sm">
                    LiteClaw memory cleared.
                  </p>
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
            </CardContent>
          </Card>
              </div>
            )}

            {!user && accessStatus !== undefined && !overlayVisible && (
              <div className="rounded-lg border border-border bg-muted/30 p-4 text-center text-sm">
                Sign in to spawn your LiteClaw.
              </div>
            )}
          </AssemblingFrame>
        </Animator>
      </div>
    </div>
  );
}
