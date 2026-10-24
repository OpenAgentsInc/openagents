/** Wait for the accepted command's state, including non-revisioned draft writes. */
export async function awaitAcceptedCommand(input: {
  receipt(): { ok: boolean; message: string; revision: number } | undefined;
  status(): { revision: number; phase: string; authoring?: { draft: string } };
  closed(): boolean;
  draft?: string;
  timeoutMs?: number;
}): Promise<void> {
  const until = performance.now() + (input.timeoutMs ?? 14000);
  while (!input.closed() && performance.now() < until) {
    const receipt = input.receipt();
    if (receipt && !receipt.ok) throw new Error(receipt.message);
    const state = input.status();
    if (state.phase === "error") throw new Error("The world could not open.");
    if (
      receipt &&
      state.revision >= receipt.revision &&
      (input.draft === undefined || state.authoring?.draft === input.draft)
    )
      return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(
    input.closed()
      ? "The world closed before the change could finish."
      : "The change took too long. Check the world before trying again.",
  );
}
