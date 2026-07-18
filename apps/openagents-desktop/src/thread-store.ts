import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"
import { randomUUID } from "node:crypto"
import { titleChatThreadFromMessage } from "@openagentsinc/khala-sync"
import { Schema } from "@effect-native/core/effect"

import {
  decode,
  DesktopThreadSchema,
  type DesktopMessage,
  type DesktopThread,
} from "./chat-contract.ts"

const maxThreads = 5
const maxNotes = 80
const titleFor = (text: string): string => text.replace(/\s+/g, " ").trim().slice(0, 48) || "New chat"
const compareDesktopThreadsByLastAccess = (left: DesktopThread, right: DesktopThread): number =>
  right.updatedAt.localeCompare(left.updatedAt) || left.id.localeCompare(right.id)

export type ThreadStoreOptions = Readonly<{
  /** Thread ids owned by nonterminal FullAutoRuns. They remain durable in
   * addition to the five-entry ordinary composer cache until terminal. */
  protectedThreadIds?: () => ReadonlySet<string>
}>

export const makeThreadStore = (file: string, options: ThreadStoreOptions = {}) => {
  const bounded = (threads: ReadonlyArray<DesktopThread>): DesktopThread[] => {
    const sorted = [...threads].sort(compareDesktopThreadsByLastAccess)
    let protectedIds: ReadonlySet<string> = new Set()
    try {
      protectedIds = options.protectedThreadIds?.() ?? protectedIds
    } catch {
      // A corrupt/unavailable auxiliary authority must not corrupt the chat
      // cache. Normal LRU behavior remains the fail-safe fallback.
    }
    const protectedThreads = sorted.filter(thread => protectedIds.has(thread.id))
    const ordinaryThreads = sorted.filter(thread => !protectedIds.has(thread.id)).slice(0, maxThreads)
    return [...protectedThreads, ...ordinaryThreads].sort(compareDesktopThreadsByLastAccess)
  }
  const read = (): DesktopThread[] => {
    try {
      const value = JSON.parse(readFileSync(file, "utf8")) as { threads?: unknown }
      const decoded = decode(Schema.Array(DesktopThreadSchema), value.threads) as ReadonlyArray<DesktopThread> | null
      return bounded(decoded?.map(thread => thread.createdAt === undefined
        ? { ...thread, createdAt: thread.updatedAt }
        : thread) ?? [])
    } catch { return [] }
  }
  const write = (threads: DesktopThread[]): DesktopThread[] => {
    // This file is the bounded mutable-composer cache, not the sidebar's
    // presentation catalog. Retain the five most recently accessed threads
    // so an older-created conversation cannot disappear while its active turn
    // is still streaming or immediately before Full Auto continues it.
    const retained = bounded(threads)
    mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 })
    if (process.platform !== "win32") chmodSync(path.dirname(file), 0o700)
    const temporary = `${file}.tmp`
    writeFileSync(temporary, JSON.stringify({ version: 1, threads: retained }), { encoding: "utf8", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(temporary, 0o600)
    renameSync(temporary, file)
    if (process.platform !== "win32") chmodSync(file, 0o600)
    return retained
  }
  return {
    list: (): DesktopThread[] => read(),
    /** Re-admit a host-verified thread projection that aged out of the
     * bounded recent set. Callers own identity/continuity verification; this
     * store only persists the supplied local thread id and bounded notes. */
    restoreThread: (thread: DesktopThread): DesktopThread => {
      const restored = {
        ...thread,
        createdAt: thread.createdAt ?? thread.updatedAt,
        notes: thread.notes.slice(-maxNotes),
      }
      write([restored, ...read().filter(candidate => candidate.id !== restored.id)])
      return restored
    },
    newThread: (title?: string): DesktopThread => {
      const createdAt = new Date().toISOString()
      const thread: DesktopThread = { id: randomUUID(), title: title ?? "New chat", createdAt, updatedAt: createdAt, notes: [] }
      write([thread, ...read()])
      return thread
    },
    /** H2: create a distinct local thread from an already-bounded, host-read
     * history seed. Every note is copied and the source store/history is never
     * addressed by this write. */
    forkThread: (seed: ReadonlyArray<DesktopMessage>): DesktopThread => {
      const notes = seed.slice(-maxNotes).map(note => ({ ...note }))
      const firstUser = notes.find(note => note.role === "user")?.text ?? "Forked conversation"
      const createdAt = new Date().toISOString()
      const thread: DesktopThread = {
        id: randomUUID(),
        title: `Fork · ${titleFor(firstUser)}`.slice(0, 55),
        createdAt,
        updatedAt: createdAt,
        notes,
      }
      write([thread, ...read()])
      return thread
    },
    open: (id: string): DesktopThread | null => read().find((thread) => thread.id === id) ?? null,
    rename: (id: string, title: string): DesktopThread | null => {
      const nextTitle = title.trim()
      if (nextTitle === "" || nextTitle.length > 120) return null
      const found = read().find((thread) => thread.id === id)
      if (!found) return null
      const next: DesktopThread = { ...found, title: nextTitle }
      write([next, ...read().filter((thread) => thread.id !== id)])
      return next
    },
    append: (id: string, message: DesktopMessage): DesktopThread | null => {
      const found = read().find((thread) => thread.id === id)
      if (!found) return null
      const next: DesktopThread = {
        ...found,
        title: message.role === "user" ? titleChatThreadFromMessage(found.title, message.text) : found.title,
        updatedAt: new Date().toISOString(),
        notes: [...found.notes, message].slice(-maxNotes),
      }
      write([next, ...read().filter((thread) => thread.id !== id)])
      return next
    },
    /** Replace one exact keyed note in place without changing transcript order. */
    upsert: (id: string, message: DesktopMessage): DesktopThread | null => {
      const found = read().find((thread) => thread.id === id)
      if (!found) return null
      const index = found.notes.findIndex(note => note.key === message.key)
      const notes = index === -1
        ? [...found.notes, message].slice(-maxNotes)
        : found.notes.map((note, noteIndex) => noteIndex === index ? message : note)
      const next: DesktopThread = {
        ...found,
        title: message.role === "user" ? titleChatThreadFromMessage(found.title, message.text) : found.title,
        updatedAt: new Date().toISOString(),
        notes,
      }
      write([next, ...read().filter((thread) => thread.id !== id)])
      return next
    },
    /** Remove one exact keyed runtime note without touching adjacent history. */
    remove: (id: string, key: string): DesktopThread | null => {
      const found = read().find((thread) => thread.id === id)
      if (!found) return null
      const next: DesktopThread = {
        ...found,
        updatedAt: new Date().toISOString(),
        notes: found.notes.filter(note => note.key !== key),
      }
      write([next, ...read().filter((thread) => thread.id !== id)])
      return next
    },
  }
}
