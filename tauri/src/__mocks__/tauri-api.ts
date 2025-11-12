export async function invoke<T = unknown>(
  _cmd: string,
  _args?: Record<string, unknown>
): Promise<T | undefined> {
  // No-op mock for Storybook/browser environment
  return undefined;
}

