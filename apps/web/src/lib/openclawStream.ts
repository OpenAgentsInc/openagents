export type StreamEvent = {
  type?: string;
  delta?: string;
  text?: string;
  error?: { message?: string };
};

export async function consumeOpenClawStream(
  stream: ReadableStream<Uint8Array>,
  onDelta: (delta: string) => void,
): Promise<void> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true }).replace(/\r\n/g, '\n');

    let splitIndex = buffer.indexOf('\n\n');
    while (splitIndex !== -1) {
      const rawEvent = buffer.slice(0, splitIndex).trim();
      buffer = buffer.slice(splitIndex + 2);
      if (rawEvent) {
        const lines = rawEvent.split('\n');
        for (const line of lines) {
          if (!line.startsWith('data:')) continue;
          const data = line.slice(5).trim();
          if (!data) continue;
          if (data === '[DONE]') return;
          let parsed: StreamEvent | null = null;
          try {
            parsed = JSON.parse(data) as StreamEvent;
          } catch {
            // ignore malformed chunks
          }
          if (!parsed) continue;
          if (parsed.error?.message) {
            throw new Error(parsed.error.message);
          }
          if (typeof parsed.delta === 'string' && parsed.delta.length > 0) {
            onDelta(parsed.delta);
            continue;
          }
          if (
            typeof parsed.text === 'string' &&
            parsed.text.length > 0 &&
            parsed.type?.includes('output_text')
          ) {
            onDelta(parsed.text);
          }
        }
      }
      splitIndex = buffer.indexOf('\n\n');
    }
  }
}
