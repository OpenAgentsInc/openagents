export async function health(): Promise<boolean> {
  const response = await fetch("/health");
  const body = (await response.json()) as { ok: boolean };
  return body.ok;
}
