import {
  AUDIO_MEDIA_MAGIC,
  AUDIO_PROTOCOL_VERSION,
  MAX_AUDIO_PAYLOAD_BYTES,
  decodeMediaHeader,
  type VoiceIdentity,
} from "@openagentsinc/audio-contract";

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

const bytesToHex = (bytes: Uint8Array): string =>
  [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");

export const bytesToBase64 = (bytes: Uint8Array): string => {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let result = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const first = bytes[index] ?? 0;
    const second = bytes[index + 1];
    const third = bytes[index + 2];
    const value = (first << 16) | ((second ?? 0) << 8) | (third ?? 0);
    result += alphabet[(value >>> 18) & 63] ?? "";
    result += alphabet[(value >>> 12) & 63] ?? "";
    result += second === undefined ? "=" : (alphabet[(value >>> 6) & 63] ?? "");
    result += third === undefined ? "=" : (alphabet[value & 63] ?? "");
  }
  return result;
};

export const base64ToBytes = (value: string): Uint8Array => {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const normalized = value.replace(/\s/gu, "");
  if (
    normalized.length === 0 ||
    normalized.length % 4 !== 0 ||
    /[^A-Za-z0-9+/=]/u.test(normalized)
  ) {
    throw new Error("The audio frame is not valid base64.");
  }
  const output: number[] = [];
  for (let index = 0; index < normalized.length; index += 4) {
    const chars = normalized.slice(index, index + 4);
    const values = [...chars].map((character) =>
      character === "=" ? 0 : alphabet.indexOf(character),
    );
    if (values.some((entry) => entry < 0)) throw new Error("The audio frame is not valid base64.");
    const combined =
      ((values[0] ?? 0) << 18) |
      ((values[1] ?? 0) << 12) |
      ((values[2] ?? 0) << 6) |
      (values[3] ?? 0);
    output.push((combined >>> 16) & 0xff);
    if (chars[2] !== "=") output.push((combined >>> 8) & 0xff);
    if (chars[3] !== "=") output.push(combined & 0xff);
  }
  return Uint8Array.from(output);
};

export type Sha256 = (bytes: Uint8Array) => Promise<Uint8Array>;

export const sha256Hex = async (bytes: Uint8Array, sha256: Sha256): Promise<string> =>
  bytesToHex(await sha256(bytes));

export const encodeNip98Authorization = (event: unknown): string =>
  `Nostr ${bytesToBase64(textEncoder.encode(JSON.stringify(event)))}`;

export const encodeClientAudioFrame = async (
  input: Readonly<{
    identity: VoiceIdentity;
    sequence: number;
    pcm: Uint8Array;
    sha256: Sha256;
  }>,
): Promise<ArrayBuffer> => {
  if (
    input.pcm.byteLength === 0 ||
    input.pcm.byteLength > MAX_AUDIO_PAYLOAD_BYTES ||
    input.pcm.byteLength % 2 !== 0
  ) {
    throw new Error("The microphone frame is outside the Sarah voice audio bounds.");
  }
  const header = textEncoder.encode(
    JSON.stringify({
      schema: AUDIO_PROTOCOL_VERSION,
      kind: "client_audio",
      identity: input.identity,
      sequence: input.sequence,
      codec: "pcm_s16le",
      sampleRateHz: 24_000,
      channels: 1,
      payloadLength: input.pcm.byteLength,
      sha256: await sha256Hex(input.pcm, input.sha256),
    }),
  );
  if (header.byteLength > 8_192) throw new Error("The microphone frame header is too large.");
  const frame = new Uint8Array(8 + header.byteLength + input.pcm.byteLength);
  frame.set(textEncoder.encode(AUDIO_MEDIA_MAGIC), 0);
  new DataView(frame.buffer).setUint32(4, header.byteLength);
  frame.set(header, 8);
  frame.set(input.pcm, 8 + header.byteLength);
  return frame.buffer;
};

export type ServerAudioFrame = Readonly<{
  sequence: number;
  itemRef: string;
  pcm: Uint8Array;
}>;

export const decodeServerAudioFrame = async (
  input: Readonly<{
    frame: ArrayBuffer;
    identity: VoiceIdentity;
    expectedSequence: number;
    sha256: Sha256;
  }>,
): Promise<ServerAudioFrame> => {
  const bytes = new Uint8Array(input.frame);
  if (bytes.byteLength < 8 || textDecoder.decode(bytes.subarray(0, 4)) !== AUDIO_MEDIA_MAGIC) {
    throw new Error("The Sarah audio frame has an invalid envelope.");
  }
  const headerLength = new DataView(bytes.buffer, bytes.byteOffset + 4, 4).getUint32(0);
  if (headerLength < 2 || headerLength > 8_192 || 8 + headerLength > bytes.byteLength) {
    throw new Error("The Sarah audio frame header is invalid.");
  }
  const header = decodeMediaHeader(
    JSON.parse(textDecoder.decode(bytes.subarray(8, 8 + headerLength))) as unknown,
  );
  const pcm = bytes.slice(8 + headerLength);
  if (
    header.kind !== "server_tts" ||
    header.codec !== "pcm_s16le" ||
    header.sampleRateHz !== 24_000 ||
    header.channels !== 1 ||
    header.sequence !== input.expectedSequence ||
    JSON.stringify(header.identity) !== JSON.stringify(input.identity) ||
    pcm.byteLength === 0 ||
    pcm.byteLength % 2 !== 0 ||
    pcm.byteLength !== header.payloadLength ||
    (await sha256Hex(pcm, input.sha256)) !== header.sha256
  ) {
    throw new Error("The Sarah audio frame does not match this voice session.");
  }
  return { sequence: header.sequence, itemRef: header.turnRef, pcm };
};
