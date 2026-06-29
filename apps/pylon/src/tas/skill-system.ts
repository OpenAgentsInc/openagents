export type SkillDescriptor = Readonly<{
  name: string
  description: string
  enabled: boolean
}>

export type SkillRegistry = Readonly<{
  skills: ReadonlyMap<string, SkillDescriptor>
}>

export type SkillInvocationResult =
  | Readonly<{ ok: true; skill: SkillDescriptor }>
  | Readonly<{ ok: false; reason: "disabled" | "unknown" }>

export function createSkillRegistry(
  skills: readonly SkillDescriptor[] = [],
): SkillRegistry {
  return skills.reduce<SkillRegistry>(
    (registry, skill) => registerSkill(registry, skill),
    { skills: new Map<string, SkillDescriptor>() },
  )
}

export function registerSkill(
  registry: SkillRegistry,
  skill: SkillDescriptor,
): SkillRegistry {
  assertSkillName(skill.name)

  if (registry.skills.has(skill.name)) {
    throw new Error(`Skill already registered: ${skill.name}`)
  }

  const skills = new Map(registry.skills)
  skills.set(skill.name, skill)

  return { skills }
}

export function resolveSkillInvocation(
  registry: SkillRegistry,
  invocationName: string,
): SkillInvocationResult {
  const skillName = explicitSkillNameFrom(invocationName)

  if (!skillName) {
    return { ok: false, reason: "unknown" }
  }

  const skill = registry.skills.get(skillName)

  if (!skill) {
    return { ok: false, reason: "unknown" }
  }

  if (!skill.enabled) {
    return { ok: false, reason: "disabled" }
  }

  return { ok: true, skill }
}

function explicitSkillNameFrom(invocationName: string): string | undefined {
  const trimmed = invocationName.trim()

  if (trimmed === "") {
    return undefined
  }

  if (trimmed.startsWith("/")) {
    const name = trimmed.slice(1)
    return isSkillName(name) ? name : undefined
  }

  return isSkillName(trimmed) ? trimmed : undefined
}

function assertSkillName(name: string): void {
  if (!isSkillName(name)) {
    throw new Error(`Invalid skill name: ${name}`)
  }
}

function isSkillName(name: string): boolean {
  return /^[A-Za-z][A-Za-z0-9_-]*$/.test(name)
}
