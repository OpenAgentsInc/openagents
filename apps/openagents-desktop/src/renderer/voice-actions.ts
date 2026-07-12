import { desktopCanonicalCommandRegistry, type DesktopCommandId } from "../desktop-command-contract.ts"

export type VoiceActionRoute =
  | Readonly<{ kind: "message"; text: string; confidence: number }>
  | Readonly<{ kind: "interrupt"; confidence: number }>
  | Readonly<{ kind: "focus"; commandId: DesktopCommandId; confidence: number }>
  | Readonly<{ kind: "editable_fallback"; text: string; confidence: number }>

type Vector = ReadonlyMap<string, number>
const embed = (input: string): Vector => {
  const normalized = `  ${input.toLocaleLowerCase().normalize("NFKC").replace(/[^\p{L}\p{N}]+/gu, " ").trim()}  `
  const counts = new Map<string, number>()
  for (let index = 0; index + 2 < normalized.length; index++) {
    const gram = normalized.slice(index, index + 3)
    counts.set(gram, (counts.get(gram) ?? 0) + 1)
  }
  const magnitude = Math.sqrt([...counts.values()].reduce((sum, value) => sum + value * value, 0)) || 1
  return new Map([...counts].map(([key, value]) => [key, value / magnitude]))
}
const cosine = (left: Vector, right: Vector): number => {
  let score = 0
  for (const [key, value] of left) score += value * (right.get(key) ?? 0)
  return score
}

const routeCatalog = [
  { kind: "interrupt" as const, descriptor: "stop interrupt cancel the current active response turn immediately" },
  { kind: "focus" as const, commandId: "chat.open" as const, descriptor: "open focus show the chat conversation" },
  { kind: "focus" as const, commandId: "workspace.files" as const, descriptor: "open focus show project files workspace" },
  { kind: "focus" as const, commandId: "workspace.home" as const, descriptor: "open focus show project home overview" },
  { kind: "focus" as const, commandId: "workspace.review" as const, descriptor: "open focus show review changes diff" },
] satisfies ReadonlyArray<Readonly<{ kind: "interrupt"; descriptor: string } | { kind: "focus"; commandId: DesktopCommandId; descriptor: string }>>

const embeddedCatalog = routeCatalog.map(route => ({ ...route, vector: embed(route.descriptor) }))
const automaticCommandIds = new Set<DesktopCommandId>(["chat.open", "workspace.files", "workspace.home", "workspace.review"])

/**
 * Central typed semantic selector for voice actions. It uses normalized
 * character-trigram embeddings and cosine similarity over a closed catalog;
 * no keyword, substring, regexp, DOM, path, shell, or model-selected route can
 * enter command authority. Low confidence remains editable text.
 */
export const selectVoiceAction = (text: string): VoiceActionRoute => {
  const bounded = text.trim().slice(0, 16_384)
  if (bounded === "") return { kind: "editable_fallback", text: "", confidence: 0 }
  const query = embed(bounded)
  const ranked = embeddedCatalog
    .map(route => ({ route, score: cosine(query, route.vector) }))
    .sort((left, right) => right.score - left.score)
  const best = ranked[0]!
  const margin = best.score - (ranked[1]?.score ?? 0)
  if (best.score < 0.43 || margin < 0.08) return { kind: "message", text: bounded, confidence: best.score }
  if (best.route.kind === "interrupt") return { kind: "interrupt", confidence: best.score }
  const registered = desktopCanonicalCommandRegistry.some(command =>
    command.id === best.route.commandId && command.arguments === "workspace" && automaticCommandIds.has(command.id))
  return registered
    ? { kind: "focus", commandId: best.route.commandId, confidence: best.score }
    : { kind: "editable_fallback", text: bounded, confidence: best.score }
}

export type VoiceFinalAdmission = Readonly<{
  sessionRef: string
  generation: number
  utteranceRef: string
  text: string
}>

export type VoiceActionPeers = Readonly<{
  submitMessage: (text: string) => Promise<void>
  interrupt: () => Promise<void>
  focusRegisteredCommand: (commandId: DesktopCommandId) => Promise<void>
  editFallback: (text: string) => Promise<void>
}>

/** Executes only through the same typed peers used by visible controls. */
export const executeVoiceAction = async (action: VoiceActionRoute, peers: VoiceActionPeers): Promise<void> => {
  if (action.kind === "message") await peers.submitMessage(action.text)
  else if (action.kind === "interrupt") await peers.interrupt()
  else if (action.kind === "focus") await peers.focusRegisteredCommand(action.commandId)
  else await peers.editFallback(action.text)
}

/** Bounded exactly-once ledger for final utterances across replay/lost ACK. */
export const makeVoiceFinalLedger = (capacity = 512) => {
  const admitted = new Set<string>()
  const order: string[] = []
  return {
    admit: (input: VoiceFinalAdmission, currentGeneration: number = input.generation): VoiceActionRoute | null => {
      if (!Number.isSafeInteger(input.generation) || input.generation < 1 || input.generation !== currentGeneration || input.sessionRef === "" || input.utteranceRef === "") return null
      const key = `${input.sessionRef}\u0000${input.generation}\u0000${input.utteranceRef}`
      if (admitted.has(key)) return null
      admitted.add(key); order.push(key)
      while (order.length > Math.max(1, capacity)) admitted.delete(order.shift()!)
      return selectVoiceAction(input.text)
    },
  }
}
