import { v } from 'convex/values';
import { internalMutation, internalQuery, query, type MutationCtx, type QueryCtx } from './_generated/server';
import type { Doc } from './_generated/dataModel';
import { fail, requireFound } from './lib/errors';

const STATUS_VALUES = new Set(['provisioning', 'ready', 'error', 'deleted']);
const ENCRYPTION_ALGO = 'AES-GCM';
const IV_BYTES = 12;

const SECRET_FIELDS = {
  service_token: {
    cipher: 'service_token_encrypted',
    iv: 'service_token_iv',
    alg: 'service_token_alg',
  },
  provider_keys: {
    cipher: 'provider_keys_encrypted',
    iv: 'provider_keys_iv',
    alg: 'provider_keys_alg',
  },
} as const;

type InstanceDoc = Doc<'openclaw_instances'>;

type EncryptedPayload = {
  ciphertext: string;
  iv: string;
  alg: string;
};

let cachedKey: CryptoKey | null = null;
let cachedKeyRaw: string | null = null;

const normalizeStatus = (status: string): string => {
  const normalized = status.trim().toLowerCase();
  if (!STATUS_VALUES.has(normalized)) {
    fail('BAD_REQUEST', `Invalid OpenClaw status: ${status}`);
  }
  return normalized;
};

const decodeHex = (value: string): Uint8Array => {
  const cleaned = value.trim();
  if (!/^[0-9a-fA-F]+$/.test(cleaned) || cleaned.length % 2 !== 0) {
    throw new Error('Invalid hex key format');
  }
  const bytes = new Uint8Array(cleaned.length / 2);
  for (let i = 0; i < cleaned.length; i += 2) {
    bytes[i / 2] = parseInt(cleaned.slice(i, i + 2), 16);
  }
  return bytes;
};

const decodeBase64 = (value: string): Uint8Array => {
  const cleaned = value.trim();
  const binary = atob(cleaned);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
};

const decodeKey = (raw: string): Uint8Array => {
  const trimmed = raw.trim();
  if (!trimmed) {
    throw new Error('OPENCLAW_ENCRYPTION_KEY is empty');
  }
  if (/^[0-9a-fA-F]{64}$/.test(trimmed)) {
    return decodeHex(trimmed);
  }
  return decodeBase64(trimmed);
};

const getEncryptionKey = async (): Promise<CryptoKey> => {
  const raw = process.env.OPENCLAW_ENCRYPTION_KEY;
  if (!raw) {
    throw new Error('OPENCLAW_ENCRYPTION_KEY not configured');
  }
  if (cachedKey && cachedKeyRaw === raw) {
    return cachedKey;
  }
  const keyBytes = decodeKey(raw);
  if (keyBytes.length !== 32) {
    throw new Error('OPENCLAW_ENCRYPTION_KEY must be 32 bytes');
  }
  const key = await crypto.subtle.importKey(
    'raw',
    keyBytes as unknown as BufferSource,
    { name: ENCRYPTION_ALGO },
    false,
    ['encrypt', 'decrypt'],
  );
  cachedKey = key;
  cachedKeyRaw = raw;
  return key;
};

const bytesToBase64 = (bytes: ArrayBuffer | Uint8Array): string => {
  const buffer = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let binary = '';
  for (const byte of buffer) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
};

const encryptValue = async (value: string): Promise<EncryptedPayload> => {
  const key = await getEncryptionKey();
  const iv = crypto.getRandomValues(new Uint8Array(IV_BYTES));
  const encoded = new TextEncoder().encode(value);
  const ciphertext = await crypto.subtle.encrypt({ name: ENCRYPTION_ALGO, iv }, key, encoded);
  return {
    ciphertext: bytesToBase64(ciphertext),
    iv: bytesToBase64(iv),
    alg: ENCRYPTION_ALGO,
  };
};

const decryptValue = async (payload: EncryptedPayload): Promise<string> => {
  if (payload.alg !== ENCRYPTION_ALGO) {
    throw new Error(`Unsupported encryption algorithm: ${payload.alg}`);
  }
  const key = await getEncryptionKey();
  const iv = decodeBase64(payload.iv);
  const ciphertext = decodeBase64(payload.ciphertext);
  const decrypted = await crypto.subtle.decrypt(
    { name: ENCRYPTION_ALGO, iv: iv as unknown as BufferSource },
    key,
    ciphertext as unknown as BufferSource,
  );
  return new TextDecoder().decode(new Uint8Array(decrypted));
};

const getSecretFields = (key: string) => {
  const normalized = key.trim().toLowerCase();
  if (normalized in SECRET_FIELDS) {
    return SECRET_FIELDS[normalized as keyof typeof SECRET_FIELDS];
  }
  return null;
};

const buildPatch = (values: Record<string, unknown>): Record<string, unknown> => {
  const patch: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(values)) {
    if (value !== undefined) {
      patch[key] = value;
    }
  }
  return patch;
};

const getInstanceByUserId = async (
  ctx: QueryCtx | MutationCtx,
  user_id: string,
): Promise<InstanceDoc | null> => {
  return ctx.db
    .query('openclaw_instances')
    .withIndex('by_user_id', (q) => q.eq('user_id', user_id))
    .first();
};

export const getInstanceForUser = internalQuery({
  args: {
    user_id: v.string(),
  },
  handler: async (ctx, args) => {
    return getInstanceByUserId(ctx, args.user_id);
  },
});

/** Public query: current user's OpenClaw instance summary (for sidebar, etc.). Returns null if not authenticated or no instance. */
export const getInstanceForCurrentUser = query({
  args: {},
  handler: async (ctx) => {
    const identity = (await ctx.auth.getUserIdentity()) as { subject?: string } | null;
    if (!identity?.subject) return null;
    const doc = await getInstanceByUserId(ctx, identity.subject);
    if (!doc) return null;
    return {
      status: doc.status,
      runtime_name: doc.runtime_name ?? null,
      created_at: doc.created_at,
      updated_at: doc.updated_at,
      last_ready_at: doc.last_ready_at ?? null,
    };
  },
});

export const upsertInstance = internalMutation({
  args: {
    user_id: v.string(),
    status: v.optional(v.string()),
    runtime_url: v.optional(v.string()),
    runtime_name: v.optional(v.string()),
    cf_account_id: v.optional(v.string()),
    cf_worker_name: v.optional(v.string()),
    cf_worker_id: v.optional(v.string()),
    cf_container_app_id: v.optional(v.string()),
    cf_container_app_name: v.optional(v.string()),
    r2_bucket_name: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const existing = await getInstanceByUserId(ctx, args.user_id);
    const now = Date.now();
    const status = args.status ? normalizeStatus(args.status) : undefined;

    if (existing) {
      const patch = buildPatch({
        status,
        runtime_url: args.runtime_url,
        runtime_name: args.runtime_name,
        cf_account_id: args.cf_account_id,
        cf_worker_name: args.cf_worker_name,
        cf_worker_id: args.cf_worker_id,
        cf_container_app_id: args.cf_container_app_id,
        cf_container_app_name: args.cf_container_app_name,
        r2_bucket_name: args.r2_bucket_name,
        updated_at: now,
      });
      if (Object.keys(patch).length > 0) {
        await ctx.db.patch(existing._id, patch);
      }
      return (await ctx.db.get(existing._id)) ?? existing;
    }

    const record: Record<string, unknown> = {
      user_id: args.user_id,
      status: status ?? 'provisioning',
      created_at: now,
      updated_at: now,
    };

    const optionalFields: Record<string, string | undefined> = {
      runtime_url: args.runtime_url,
      runtime_name: args.runtime_name,
      cf_account_id: args.cf_account_id,
      cf_worker_name: args.cf_worker_name,
      cf_worker_id: args.cf_worker_id,
      cf_container_app_id: args.cf_container_app_id,
      cf_container_app_name: args.cf_container_app_name,
      r2_bucket_name: args.r2_bucket_name,
    };
    for (const [key, value] of Object.entries(optionalFields)) {
      if (value !== undefined) {
        record[key] = value;
      }
    }

    const instanceId = await ctx.db.insert('openclaw_instances', record as InstanceDoc);
    const instance = await ctx.db.get(instanceId);
    return requireFound(instance, 'NOT_FOUND', 'Instance not found after creation');
  },
});

export const setInstanceStatus = internalMutation({
  args: {
    user_id: v.string(),
    status: v.string(),
  },
  handler: async (ctx, args) => {
    const instance = await getInstanceByUserId(ctx, args.user_id);
    const record = requireFound(instance, 'NOT_FOUND', 'Instance not found');
    const now = Date.now();
    const normalized = normalizeStatus(args.status);

    const patch: Record<string, unknown> = {
      status: normalized,
      updated_at: now,
    };
    if (normalized === 'ready') {
      patch.last_ready_at = now;
    }

    await ctx.db.patch(record._id, patch);
    return (await ctx.db.get(record._id)) ?? record;
  },
});

export const storeEncryptedSecret = internalMutation({
  args: {
    user_id: v.string(),
    key: v.string(),
    value: v.string(),
  },
  handler: async (ctx, args) => {
    const instance = await getInstanceByUserId(ctx, args.user_id);
    const record = requireFound(instance, 'NOT_FOUND', 'Instance not found');
    const fields = getSecretFields(args.key);
    if (!fields) {
      fail('BAD_REQUEST', `Unsupported secret key: ${args.key}`);
    }
    const f = fields!;

    const encrypted = await encryptValue(args.value);
    const patch = {
      [f.cipher]: encrypted.ciphertext,
      [f.iv]: encrypted.iv,
      [f.alg]: encrypted.alg,
      updated_at: Date.now(),
    } as Record<string, unknown>;

    await ctx.db.patch(record._id, patch);
    return { ok: true };
  },
});

export const getDecryptedSecret = internalQuery({
  args: {
    user_id: v.string(),
    key: v.string(),
  },
  handler: async (ctx, args) => {
    const instance = await getInstanceByUserId(ctx, args.user_id);
    if (!instance) {
      return null;
    }
    const fields = getSecretFields(args.key);
    if (!fields) {
      return null;
    }
    const f = fields!;

    const record = instance as unknown as Record<string, string | null | undefined>;
    const ciphertext = record[f.cipher];
    const iv = record[f.iv];
    const alg = record[f.alg] ?? ENCRYPTION_ALGO;

    if (!ciphertext || !iv) {
      return null;
    }

    return decryptValue({ ciphertext, iv, alg });
  },
});
