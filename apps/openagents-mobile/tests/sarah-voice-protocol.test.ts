import { createHash } from "node:crypto";
import { describe, expect, test } from "vite-plus/test";

import {
  AUDIO_MEDIA_MAGIC,
  AUDIO_PROTOCOL_VERSION,
  type VoiceIdentity,
} from "@openagentsinc/audio-contract";

import {
  bytesToBase64,
  decodeServerAudioFrame,
  encodeClientAudioFrame,
} from "../src/sarah-voice/protocol.ts";

const identity: VoiceIdentity = {
  ownerRef: "user-1",
  deviceRef: "mobile-1",
  threadRef: "thread-1",
  sessionRef: "session-1",
  generation: 1,
};

const sha256 = async (bytes: Uint8Array): Promise<Uint8Array> =>
  new Uint8Array(createHash("sha256").update(bytes).digest());

const makeServerFrame = async (
  pcm: Uint8Array,
  sequence = 0,
): Promise<ArrayBuffer> => {
  const header = new TextEncoder().encode(
    JSON.stringify({
      schema: AUDIO_PROTOCOL_VERSION,
      kind: "server_tts",
      identity,
      sequence,
      turnRef: "provider-item-1",
      speechRef: "provider-item-1",
      codec: "pcm_s16le",
      sampleRateHz: 24_000,
      channels: 1,
      payloadLength: pcm.byteLength,
      sha256: createHash("sha256").update(pcm).digest("hex"),
    }),
  );
  const frame = new Uint8Array(8 + header.byteLength + pcm.byteLength);
  frame.set(new TextEncoder().encode(AUDIO_MEDIA_MAGIC), 0);
  new DataView(frame.buffer).setUint32(4, header.byteLength);
  frame.set(header, 8);
  frame.set(pcm, 8 + header.byteLength);
  return frame.buffer;
};

describe("OpenAgents Sarah mobile OAA1 transport", () => {
  test("encodes bounded 24 kHz mono PCM with the exact digest", async () => {
    const frame = new Uint8Array(
      await encodeClientAudioFrame({
        identity,
        sequence: 3,
        pcm: Uint8Array.from([1, 0, 2, 0]),
        sha256,
      }),
    );
    expect(new TextDecoder().decode(frame.subarray(0, 4))).toBe("OAA1");
    const headerLength = new DataView(frame.buffer).getUint32(4);
    const header = JSON.parse(
      new TextDecoder().decode(frame.subarray(8, 8 + headerLength)),
    ) as Record<string, unknown>;
    expect(header).toMatchObject({
      schema: AUDIO_PROTOCOL_VERSION,
      kind: "client_audio",
      identity,
      sequence: 3,
      codec: "pcm_s16le",
      sampleRateHz: 24_000,
      channels: 1,
      payloadLength: 4,
    });
    expect(header.sha256).toBe(
      createHash("sha256").update(frame.subarray(8 + headerLength)).digest("hex"),
    );
  });

  test("rejects malformed microphone and server frames", async () => {
    await expect(
      encodeClientAudioFrame({
        identity,
        sequence: 0,
        pcm: Uint8Array.from([1]),
        sha256,
      }),
    ).rejects.toThrow("outside");

    const frame = await makeServerFrame(Uint8Array.from([1, 0, 2, 0]));
    await expect(
      decodeServerAudioFrame({
        frame,
        identity,
        expectedSequence: 1,
        sha256,
      }),
    ).rejects.toThrow("does not match");

    const tampered = new Uint8Array(frame.slice(0));
    const lastByte = tampered.at(-1);
    if (lastByte === undefined) throw new Error("The test frame is unexpectedly empty.");
    tampered[tampered.length - 1] = lastByte ^ 0xff;
    await expect(
      decodeServerAudioFrame({
        frame: tampered.buffer,
        identity,
        expectedSequence: 0,
        sha256,
      }),
    ).rejects.toThrow("does not match");
  });

  test("decodes validated Sarah output and uses a native-safe base64 form", async () => {
    const pcm = Uint8Array.from([1, 0, 2, 0]);
    const decoded = await decodeServerAudioFrame({
      frame: await makeServerFrame(pcm),
      identity,
      expectedSequence: 0,
      sha256,
    });
    expect(decoded).toEqual({
      sequence: 0,
      itemRef: "provider-item-1",
      pcm,
    });
    expect(bytesToBase64(pcm)).toBe("AQACAA==");
  });
});
