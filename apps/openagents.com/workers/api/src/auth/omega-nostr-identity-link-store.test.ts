import { describe, expect, test } from 'vitest'

import { makeLedgerSqliteDb } from '../test/payments-ledger-sqlite'
import { linkOmegaNostrIdentity } from './omega-nostr-identity-link-store'

const schema = `
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  primary_email TEXT,
  status TEXT NOT NULL,
  deleted_at TEXT
);
CREATE TABLE auth_identities (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  provider_subject TEXT NOT NULL,
  provider_username TEXT,
  email TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  UNIQUE(provider, provider_subject)
);
`

const pubkey = 'a'.repeat(64)
const nowIso = '2026-07-28T18:00:00.000Z'

const makeDb = () => {
  const db = makeLedgerSqliteDb(schema)
  db.raw
    .prepare(
      `INSERT INTO users
        (id, display_name, primary_email, status, deleted_at)
       VALUES (?, ?, ?, 'active', NULL), (?, ?, ?, 'active', NULL)`,
    )
    .run(
      'github:one',
      'One',
      'one@example.com',
      'github:two',
      'Two',
      'two@example.com',
    )
  return db
}

const link = (db: ReturnType<typeof makeDb>, ownerRef: string, key = pubkey) =>
  linkOmegaNostrIdentity(db, {
    displayName: 'Omega device',
    nowIso,
    ownerRef,
    pubkey: key,
  })

describe('Omega Nostr identity link store', () => {
  test('is idempotent for one active owner and never changes the owner', async () => {
    const db = makeDb()
    await expect(link(db, 'github:one')).resolves.toBe('Linked')
    await expect(link(db, 'github:one')).resolves.toBe('AlreadyLinked')
    await expect(link(db, 'github:two')).resolves.toBe('Conflict')

    const rows = await db.query(
      `SELECT user_id FROM auth_identities
        WHERE provider = 'nostr' AND provider_subject = ?`,
      [pubkey],
    )
    expect(rows).toEqual([{ user_id: 'github:one' }])
  })

  test('rejects a tombstoned binding without reactivation or reassignment', async () => {
    const db = makeDb()
    db.raw
      .prepare(
        `INSERT INTO auth_identities
          (id, user_id, provider, provider_subject, provider_username, email,
           created_at, updated_at, deleted_at)
         VALUES (?, ?, 'nostr', ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        `auth_identity_nostr_${pubkey}`,
        'github:one',
        pubkey,
        'Omega device',
        'one@example.com',
        nowIso,
        nowIso,
        nowIso,
      )

    await expect(link(db, 'github:one')).resolves.toBe('Conflict')
    await expect(link(db, 'github:two')).resolves.toBe('Conflict')
    const rows = await db.query(
      `SELECT user_id, deleted_at FROM auth_identities
        WHERE provider = 'nostr' AND provider_subject = ?`,
      [pubkey],
    )
    expect(rows).toEqual([{ deleted_at: nowIso, user_id: 'github:one' }])
  })

  test('keeps one winner across concurrent account link attempts', async () => {
    const db = makeDb()
    const owners = Array.from({ length: 8 }, (_, index) =>
      index % 2 === 0 ? 'github:one' : 'github:two',
    )
    const results = await Promise.all(
      owners.map(ownerRef => link(db, ownerRef)),
    )
    expect(results.filter(result => result === 'Linked').length).toBeGreaterThan(
      0,
    )

    const rows = await db.query(
      `SELECT user_id FROM auth_identities
        WHERE provider = 'nostr' AND provider_subject = ?`,
      [pubkey],
    )
    expect(rows).toHaveLength(1)
    const winner = String(rows[0]?.user_id)
    results.forEach((result, index) => {
      expect(
        owners[index] === winner
          ? result === 'Linked' || result === 'AlreadyLinked'
          : result === 'Conflict',
      ).toBe(true)
    })
  })
})
