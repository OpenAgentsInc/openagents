import type { LocalSkillInvocation, PluginConfigView } from "../plugin-config-contract.ts"

export type ExplicitSkillParse =
  | Readonly<{ kind: "none"; message: string }>
  | Readonly<{ kind: "invalid" }>
  | Readonly<{ kind: "skill"; message: string; skill: LocalSkillInvocation }>

/**
 * Explicit modeled slash grammar. This is not intent/keyword routing: only a
 * leading `/skill <plugin>/<skill> ` selects the already-loaded typed catalog.
 */
export const parseExplicitSkillInvocation = (
  message: string,
  plugins: ReadonlyArray<PluginConfigView>,
): ExplicitSkillParse => {
  if (!message.startsWith("/skill")) return { kind: "none", message }
  const match = /^\/skill\s+([A-Za-z0-9][A-Za-z0-9._-]{0,79})\/([A-Za-z0-9][A-Za-z0-9._-]{0,79})(?:\s+([\s\S]+))?$/.exec(message.trim())
  if (match === null) return { kind: "invalid" }
  const plugin = plugins.find(candidate => candidate.name === match[1] && candidate.enabled && candidate.readiness === "ready" && candidate.skills.includes(match[2]!))
  const prompt = (match[3] ?? "").trim()
  return plugin === undefined || prompt === ""
    ? { kind: "invalid" }
    : { kind: "skill", message: prompt, skill: { pluginRef: plugin.ref, name: match[2]! } }
}
