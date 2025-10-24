#!/usr/bin/env node
// Ingest ~/.openagents/convex/mirror/spool.jsonl into Convex via HTTP client.
// Requires `convex` devDependency present in this repo.

import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { fileURLToPath } from 'node:url'

// Convex HTTP client for Node
import { ConvexHttpClient } from 'convex/browser'

function expandHome(p) {
  if (!p) return p
  if (p === '~') return os.homedir()
  if (p.startsWith('~/')) return path.join(os.homedir(), p.slice(2))
  return p
}

const CONVEX_URL = process.env.CONVEX_URL || process.env.CONVEX_SELF_HOSTED_URL || 'http://127.0.0.1:7788'
const SPOOL = expandHome(process.env.SPOOL || '~/.openagents/convex/mirror/spool.jsonl')

async function readLines(p) {
  try {
    const data = await fs.readFile(p, 'utf8')
    return data.split(/\r?\n/).filter((l) => l.trim().length > 0)
  } catch (e) {
    if (e && e.code === 'ENOENT') return []
    throw e
  }
}

async function writeLines(p, lines) {
  const dir = path.dirname(p)
  await fs.mkdir(dir, { recursive: true })
  const tmp = p + '.tmp'
  await fs.writeFile(tmp, lines.map((l) => (typeof l === 'string' ? l : JSON.stringify(l))).join('\n') + (lines.length ? '\n' : ''))
  await fs.rename(tmp, p)
}

async function ingest() {
  const lines = await readLines(SPOOL)
  if (lines.length === 0) {
    console.log(JSON.stringify({ type: 'convex.ingest', status: 'empty' }))
    return
  }
  const client = new ConvexHttpClient(CONVEX_URL)
  let ok = 0
  const failed = []
  for (const line of lines) {
    let ev
    try { ev = JSON.parse(line) } catch { failed.push(line); continue }
    try {
      if (ev.type === 'thread_upsert') {
        const args = {
          threadId: ev.thread_id,
          title: ev.title || undefined,
          projectId: ev.project_id || undefined,
          createdAt: typeof ev.created_at === 'number' ? ev.created_at : undefined,
          updatedAt: typeof ev.updated_at === 'number' ? ev.updated_at : undefined,
        }
        await client.mutation('threads:upsertFromStream', args)
        ok++
      } else if (ev.type === 'message_create') {
        const args = { threadId: ev.thread_id, role: ev.role, text: ev.text, ts: ev.ts }
        await client.mutation('messages:create', args)
        ok++
      } else {
        // Unknown line; drop it
      }
    } catch (e) {
      failed.push(line)
    }
  }
  // Rewrite the spool with failed lines only
  await writeLines(SPOOL, failed)
  console.log(JSON.stringify({ type: 'convex.ingest', status: 'done', ok, failed: failed.length }))
}

ingest().catch((e) => { console.error(e?.stack || String(e)); process.exit(1) })

