import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { api } from '../../../convex/_generated/api';
import { buildLiteclawUrl } from '@/lib/liteclawWorker';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';

type ConnectStatus = 'idle' | 'authorizing' | 'waiting' | 'success' | 'error';

const DEVICE_METHOD = 1;

const extractDeviceCode = (instructions: string | null) => {
  if (!instructions) return null;
  const match = instructions.match(/code[:\s]+([A-Z0-9-]+)/i);
  return match?.[1] ?? null;
};

export function CodexConnectDialog() {
  const { user } = useAuth();
  const liteclawThreadId = useQuery(api.threads.getLiteclawThread);
  const getOrCreateLiteclawThread = useMutation(
    api.threads.getOrCreateLiteclawThread,
  );
  const [open, setOpen] = useState(false);
  const [status, setStatus] = useState<ConnectStatus>('idle');
  const [instructions, setInstructions] = useState<string | null>(null);
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const deviceCode = useMemo(
    () => extractDeviceCode(instructions),
    [instructions],
  );

  const resetState = useCallback(() => {
    abortRef.current?.abort();
    abortRef.current = null;
    setStatus('idle');
    setInstructions(null);
    setAuthUrl(null);
    setError(null);
  }, []);

  useEffect(() => {
    if (!open) resetState();
  }, [open, resetState]);

  const startFlow = useCallback(async () => {
    if (status === 'authorizing' || status === 'waiting') return;
    setStatus('authorizing');
    setError(null);

    try {
      const threadId =
        liteclawThreadId ?? (await getOrCreateLiteclawThread({}));
      const tokenResponse = await fetch('/codex-token', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ threadId }),
      });
      const tokenBody = (await tokenResponse.json().catch(() => null)) as
        | { ok: true; token: string }
        | { ok: false; error?: string }
        | null;
      if (!tokenResponse.ok || !tokenBody || tokenBody.ok !== true) {
        throw new Error(tokenBody?.error ?? 'Failed to create auth session.');
      }

      const authorizeResponse = await fetch(
        buildLiteclawUrl(
          `/api/sandbox/${threadId}/opencode/provider/openai/oauth/authorize`,
        ),
        {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            authorization: `Bearer ${tokenBody.token}`,
          },
          body: JSON.stringify({ method: DEVICE_METHOD }),
        },
      );
      const authorizeBody = (await authorizeResponse.json().catch(() => null)) as
        | { url?: string; instructions?: string }
        | { error?: string }
        | null;
      if (!authorizeResponse.ok) {
        throw new Error(authorizeBody?.error ?? 'Codex authorize failed.');
      }

      setAuthUrl(authorizeBody?.url ?? null);
      setInstructions(authorizeBody?.instructions ?? null);
      setStatus('waiting');

      const controller = new AbortController();
      abortRef.current = controller;

      const callbackResponse = await fetch(
        buildLiteclawUrl(
          `/api/sandbox/${threadId}/opencode/provider/openai/oauth/callback`,
        ),
        {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            authorization: `Bearer ${tokenBody.token}`,
          },
          body: JSON.stringify({ method: DEVICE_METHOD }),
          signal: controller.signal,
        },
      );
      const callbackBody = (await callbackResponse.json().catch(() => null)) as
        | { ok?: boolean; error?: string }
        | null;
      if (!callbackResponse.ok) {
        throw new Error(callbackBody?.error ?? 'Codex callback failed.');
      }

      setStatus('success');
    } catch (err) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        return;
      }
      const message = err instanceof Error ? err.message : 'Codex auth failed.';
      setError(message);
      setStatus('error');
    }
  }, [getOrCreateLiteclawThread, liteclawThreadId, status]);

  const disabled = !user || liteclawThreadId === undefined;

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          onClick={() => setOpen(true)}
          disabled={disabled}
        >
          Connect Codex
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Connect ChatGPT (Codex)</DialogTitle>
          <DialogDescription>
            Use the device code flow to connect your ChatGPT subscription.
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3 text-sm">
          {status === 'idle' && (
            <Button onClick={startFlow}>Start device flow</Button>
          )}
          {status === 'authorizing' && (
            <p className="text-muted-foreground">
              Starting device flow…
            </p>
          )}
          {status === 'waiting' && (
            <>
              <p className="text-muted-foreground">
                Visit the URL below and enter the code to finish sign-in.
              </p>
              {authUrl && (
                <a
                  className="text-primary underline"
                  href={authUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  {authUrl}
                </a>
              )}
              {deviceCode && (
                <div className="rounded-md border bg-muted px-3 py-2 font-mono text-base">
                  {deviceCode}
                </div>
              )}
              {instructions && !deviceCode && (
                <p className="text-muted-foreground">{instructions}</p>
              )}
              <p className="text-muted-foreground">
                Waiting for confirmation…
              </p>
            </>
          )}
          {status === 'success' && (
            <p className="text-emerald-600">Codex connected successfully.</p>
          )}
          {status === 'error' && (
            <>
              <p className="text-destructive">
                {error ?? 'Codex connection failed.'}
              </p>
              <Button variant="outline" onClick={startFlow}>
                Try again
              </Button>
            </>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
