import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"
import { randomUUID } from "node:crypto"

import type { DesktopMessage, DesktopThread } from "./chat-contract.ts"

const maxThreads = 5
const maxNotes = 80
const titleFor = (text: string): string => text.replace(/\s+/g, " ").trim().slice(0, 48) || "New chat"

export const makeThreadStore = (file: string) => {
  const read = (): DesktopThread[] => {
    try {
      const value = JSON.parse(readFileSync(file, "utf8")) as { threads?: DesktopThread[] }
      return Array.isArray(value.threads) ? value.threads.slice(0, maxThreads) : []
    } catch { return [] }
  }
  const write = (threads: DesktopThread[]): DesktopThread[] => {
    const bounded = [...threads].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)).slice(0, maxThreads)
    mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 })
    if (process.platform !== "win32") chmodSync(path.dirname(file), 0o700)
    const temporary = `${file}.tmp`
    writeFileSync(temporary, JSON.stringify({ version: 1, threads: bounded }), { encoding: "utf8", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(temporary, 0o600)
    renameSync(temporary, file)
    if (process.platform !== "win32") chmodSync(file, 0o600)
    return bounded
  }
  return {
    list: (): DesktopThread[] => read(),
    newThread: (): DesktopThread => {
      const thread: DesktopThread = { id: randomUUID(), title: "New chat", updatedAt: new Date().toISOString(), notes: [] }
      write([thread, ...read()])
      return thread
    },
    /** H2: create a distinct local thread from an already-bounded, host-read
     * history seed. Every note is copied and the source store/history is never
     * addressed by this write. */
    forkThread: (seed: ReadonlyArray<DesktopMessage>): DesktopThread => {
      const notes = seed.slice(-maxNotes).map(note => ({ ...note }))
      const firstUser = notes.find(note => note.role === "user")?.text ?? "Forked conversation"
      const thread: DesktopThread = {
        id: randomUUID(),
        title: `Fork · ${titleFor(firstUser)}`.slice(0, 55),
        updatedAt: new Date().toISOString(),
        notes,
      }
      write([thread, ...read()])
      return thread
    },
    open: (id: string): DesktopThread | null => read().find((thread) => thread.id === id) ?? null,
    append: (id: string, message: DesktopMessage): DesktopThread | null => {
      const found = read().find((thread) => thread.id === id)
      if (!found) return null
      const next: DesktopThread = {
        ...found,
        title: found.title === "New chat" && message.role === "user" ? titleFor(message.text) : found.title,
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
        updatedAt: new Date().toISOString(),
        notes,
      }
      write([next, ...read().filter((thread) => thread.id !== id)])
      return next
    },
  }
}
