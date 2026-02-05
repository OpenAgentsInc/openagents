import { createFileRoute } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Buffer } from 'buffer';
import { api } from '../../convex/_generated/api';
import { getConvexHttpClient } from '@/lib/convexHttpClient';

const TOKEN_TTL_MS = 5 * 60 * 1000;

const textEncoder = new TextEncoder();

const toHex = (buffer: ArrayBuffer) =>
  [...new Uint8Array(buffer)]
    .map((value) => value.toString(16).padStart(2, '0'))
    .join('');

const encodeBase64 = (bytes: Uint8Array) => {
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  if (typeof btoa === 'function') {
    return btoa(binary);
  }
  return Buffer.from(binary, 'binary').toString('base64');
};

const encodeBase64Url = (value: string) =>
  encodeBase64(textEncoder.encode(value))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/g, '');

const hmacSha256 = async (secret: string, payload: string) => {
  const key = await crypto.subtle.importKey(
    'raw',
    textEncoder.encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const signature = await crypto.subtle.sign(
    'HMAC',
    key,
    textEncoder.encode(payload),
  );
  return toHex(signature);
};

const signSandboxToken = async (
  secret: string,
  payload: { thread_id: string; user_id: string; exp: number },
) => {
  const payloadPart = encodeBase64Url(JSON.stringify(payload));
  const signature = await hmacSha256(secret, payloadPart);
  return `${payloadPart}.${signature}`;
};

export const Route = createFileRoute('/codex-token')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const body = (await request.json().catch(() => null)) as
          | { threadId?: unknown }
          | null;
        const threadId =
          body && typeof body.threadId === 'string' ? body.threadId.trim() : '';
        if (!threadId) {
          return Response.json(
            { ok: false, error: 'thread_id_required' },
            { status: 400 },
          );
        }

        const auth = await getAuth().catch(() => null);
        const userId = auth?.user?.id ?? null;
        const accessToken = auth?.accessToken ?? null;
        if (!userId || !accessToken) {
          return Response.json(
            { ok: false, error: 'not_authenticated' },
            { status: 401 },
          );
        }

        const secret = process.env.LITECLAW_CODEX_SECRET;
        if (!secret) {
          return Response.json(
            { ok: false, error: 'codex_secret_missing' },
            { status: 500 },
          );
        }

        const client = getConvexHttpClient();
        client.setAuth(accessToken);
        const thread = await client.query(api.threads.get, { threadId });
        if (!thread) {
          return Response.json(
            { ok: false, error: 'thread_not_found' },
            { status: 403 },
          );
        }

        const expiresAt = Date.now() + TOKEN_TTL_MS;
        const token = await signSandboxToken(secret, {
          thread_id: threadId,
          user_id: userId,
          exp: expiresAt,
        });

        return Response.json({
          ok: true,
          token,
          expires_at: expiresAt,
        });
      },
    },
  },
});
