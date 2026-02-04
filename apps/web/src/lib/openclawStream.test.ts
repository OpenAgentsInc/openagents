import { describe, expect, it } from 'vitest';
import { consumeOpenClawStream } from './openclawStream';

const encoder = new TextEncoder();

function streamFromChunks(chunks: string[]): ReadableStream<Uint8Array> {
  return new ReadableStream<Uint8Array>({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(encoder.encode(chunk));
      }
      controller.close();
    },
  });
}

describe('consumeOpenClawStream', () => {
  it('collects deltas and output_text fragments', async () => {
    const stream = streamFromChunks([
      'data: {"delta":"Hello"}\n\n',
      'data: {"type":"output_text.delta","text":" world"}\n\n',
      'data: [DONE]\n\n',
    ]);
    const parts: string[] = [];
    await consumeOpenClawStream(stream, (delta) => parts.push(delta));
    expect(parts.join('')).toBe('Hello world');
  });

  it('throws when error payload is received', async () => {
    const stream = streamFromChunks(['data: {"error":{"message":"boom"}}\n\n']);
    await expect(
      consumeOpenClawStream(stream, () => undefined),
    ).rejects.toThrow('boom');
  });

  it('ignores malformed JSON lines', async () => {
    const stream = streamFromChunks([
      'data: {not-json}\n\n',
      'data: {"delta":"ok"}\n\n',
      'data: [DONE]\n\n',
    ]);
    const parts: string[] = [];
    await consumeOpenClawStream(stream, (delta) => parts.push(delta));
    expect(parts).toEqual(['ok']);
  });
});
