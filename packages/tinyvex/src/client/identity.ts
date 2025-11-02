export interface ResolveResult { thread_id: string }

// Placeholder for WS-driven resolution; for now, echo back the alias as canonical.
export async function resolveAlias(idOrAlias: string): Promise<ResolveResult> {
  return { thread_id: idOrAlias };
}

