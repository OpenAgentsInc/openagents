import { useState } from 'react';
import { Link, useNavigate } from '@tanstack/react-router';
import { useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { api } from '../../../convex/_generated/api';
import { posthogCapture } from '@/lib/posthog';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { MessageSquareIcon, ServerIcon } from 'lucide-react';

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function HatcheryFlowDemo() {
  const { user, loading: authLoading } = useAuth();
  const accessStatus = useQuery(api.access.getStatus);
  const liteclawThreadId = useQuery(api.threads.getLiteclawThread);
  const createThread = useMutation(api.threads.create);
  const [email, setEmail] = useState('');
  const [waitlistStatus, setWaitlistStatus] = useState<'idle' | 'submitting' | 'success' | 'error'>('idle');
  const [waitlistError, setWaitlistError] = useState<string | null>(null);
  const joinWaitlistMutation = useMutation(api.waitlist.joinWaitlist);
  const [spawnStatus, setSpawnStatus] = useState<'idle' | 'spawning' | 'ready' | 'error'>('idle');
  const [spawnError, setSpawnError] = useState<string | null>(null);
  const navigate = useNavigate();

  const accessAllowed = accessStatus?.allowed === true;
  const waitlistEntry = accessStatus?.waitlistEntry ?? null;
  const waitlistApproved = accessStatus?.waitlistApproved === true;
  const overlayVisible = !accessAllowed;
  const hasLiteclaw = liteclawThreadId != null;

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
      const threadId = await createThread({ title: 'LiteClaw', kind: 'liteclaw' });
      setSpawnStatus('ready');
      posthogCapture('hatchery_liteclaw_spawn');
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
    <div className="flex min-h-0 flex-1 flex-col p-4">
      {/* Waitlist overlay */}
      {overlayVisible && (
        <div className="flex min-h-[60vh] flex-col items-center justify-center gap-6 rounded-lg border border-border bg-card p-8 text-center">
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
        <div className="mx-auto flex w-full max-w-2xl flex-col gap-6">
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
                </div>
                {spawnError && (
                  <p className="text-destructive text-sm">{spawnError}</p>
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
    </div>
  );
}
