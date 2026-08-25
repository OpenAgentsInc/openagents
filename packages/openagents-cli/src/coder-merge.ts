/**
 * Run several event streams at once and yield what they produce as it arrives.
 *
 * A model that asks for two tools in one turn is saying they do not depend on
 * each other. Running them in order anyway made a session that fanned out to two
 * models take the sum of both — three minutes and thirty-eight seconds, then
 * eleven — and report that they had run in parallel, which was the reasonable
 * thing to believe and not true.
 *
 * Order within one stream is preserved, because a tool call and its result are
 * a sequence. Order between streams is arrival order, which is the point.
 */
export async function* merge<T>(streams: ReadonlyArray<AsyncIterable<T>>): AsyncIterable<T> {
  if (streams.length === 0) return;
  if (streams.length === 1) {
    yield* streams[0] as AsyncIterable<T>;
    return;
  }

  const queue: T[] = [];
  let live = streams.length;
  let failure: unknown;
  let wake: (() => void) | undefined;

  const nudge = () => {
    wake?.();
    wake = undefined;
  };

  for (const stream of streams) {
    void (async () => {
      try {
        for await (const item of stream) {
          queue.push(item);
          nudge();
        }
      } catch (cause) {
        // The first failure is kept and raised once every stream has stopped:
        // a tool that threw must not strand the others mid-flight, and a caller
        // that saw half a turn's events and no error would report a turn that
        // did not happen.
        failure ??= cause;
      } finally {
        live -= 1;
        nudge();
      }
    })();
  }

  while (live > 0 || queue.length > 0) {
    while (queue.length > 0) {
      const next = queue.shift();
      if (next !== undefined) yield next;
    }
    if (live === 0) break;
    await new Promise<void>((resolve) => {
      wake = resolve;
    });
  }

  if (failure !== undefined) throw failure;
}
