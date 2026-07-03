export const khalaCodeUnsafeTextPattern =
  /\/Users\/|\/home\/|~\/|auth\.json|bearer|credential|provider[_-]?payload|raw[_-]?(prompt|trace|log|provider)|secret|sk-[a-z0-9]|api[_-]?key|cookie|authorization/i

const legacyDeadEndPattern =
  /codex_spawn_failed: No Pylon Codex assignment capacity is available right now|0\/1 available/i

export const assertKhalaCodePublicSafeText = (
  text: string,
  label = "Khala Code QA",
): void => {
  if (khalaCodeUnsafeTextPattern.test(text)) {
    throw new Error(`${label} rendered private or raw material`)
  }
  if (legacyDeadEndPattern.test(text)) {
    throw new Error(`${label} regressed to the legacy 0/1 capacity dead-end`)
  }
}

export const assertKhalaCodePublicSafeValue = (
  value: unknown,
  label = "Khala Code QA",
): void => {
  const visit = (node: unknown): void => {
    if (typeof node === "string") {
      assertKhalaCodePublicSafeText(node, label)
      return
    }
    if (node === null || typeof node !== "object") return
    if (Array.isArray(node)) {
      for (const entry of node) visit(entry)
      return
    }
    for (const [key, entry] of Object.entries(node as Record<string, unknown>)) {
      assertKhalaCodePublicSafeText(key, label)
      visit(entry)
    }
  }
  visit(value)
}

export const assertKhalaCodePagePublicSafe = async (
  page: { locator: (selector: string) => { textContent: () => Promise<string | null> } },
  label = "Khala Code QA",
): Promise<void> => {
  const text = await page.locator("body").textContent()
  assertKhalaCodePublicSafeText(text ?? "", label)
}
