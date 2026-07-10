import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
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
    mkdirSync(path.dirname(file), { recursive: true })
    const temporary = `${file}.tmp`
    writeFileSync(temporary, JSON.stringify({ version: 1, threads: bounded }), "utf8")
    renameSync(temporary, file)
    return bounded
  }
  return {
    list: (): DesktopThread[] => read(),
    newThread: (): DesktopThread => {
      const thread: DesktopThread = { id: randomUUID(), title: "New chat", updatedAt: new Date().toISOString(), notes: [] }
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
  }
}
