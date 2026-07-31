import { SARAH_VOICE_PROTOCOL_VERSION, type VoiceIdentity } from "@openagentsinc/audio-contract";
import { describe, expect, test, vi } from "vite-plus/test";
import type { ClientOptions, RawData } from "ws";
import {
  openSarahLiveKitAcceptanceControlChannel,
  type SarahLiveKitAcceptanceControlSocket,
  type SarahLiveKitAcceptanceControlSocketFactory,
} from "./acceptance-livekit.js";

const identity: VoiceIdentity = {
  ownerRef: "owner-private",
  deviceRef: "device-private",
  threadRef: "thread-private",
  sessionRef: "session-private",
  generation: 3,
};

class FakeControlSocket implements SarahLiveKitAcceptanceControlSocket {
  readonly sent: string[] = [];
  readonly terminate = vi.fn();
  private openListener: (() => void) | undefined;
  private messageListener: ((data: RawData, isBinary: boolean) => void) | undefined;
  private errorListener: (() => void) | undefined;
  private closeListener: (() => void) | undefined;

  onOpen(listener: () => void): void {
    this.openListener = listener;
  }

  onMessage(listener: (data: RawData, isBinary: boolean) => void): void {
    this.messageListener = listener;
  }

  onError(listener: () => void): void {
    this.errorListener = listener;
  }

  onClose(listener: () => void): void {
    this.closeListener = listener;
  }

  send(message: string): void {
    this.sent.push(message);
  }

  open(): void {
    this.openListener?.();
  }

  message(value: unknown, isBinary = false): void {
    this.messageListener?.(Buffer.from(JSON.stringify(value)), isBinary);
  }

  error(): void {
    this.errorListener?.();
  }

  closed(): void {
    this.closeListener?.();
  }
}

const serverControl = (
  sequence: number,
  control: Readonly<Record<string, unknown>>,
): Readonly<Record<string, unknown>> => ({
  schema: SARAH_VOICE_PROTOCOL_VERSION,
  identity,
  sequence,
  ...control,
});

const setup = () => {
  const socket = new FakeControlSocket();
  let observedUrl: string | undefined;
  let observedOptions: ClientOptions | undefined;
  const factory: SarahLiveKitAcceptanceControlSocketFactory = (url, options) => {
    observedUrl = url;
    observedOptions = options;
    return socket;
  };
  let now = 100;
  const channel = openSarahLiveKitAcceptanceControlChannel(
    {
      gatewayUrl: "wss://openagents.com/api/omega/sarah/voice/connect",
      ticket: "acceptance-ticket-must-stay-private",
      identity,
      disclosureRef: "disclosure.sarah_livekit_acceptance.v1",
    },
    () => now,
    factory,
  );
  return {
    channel,
    socket,
    observed: () => ({ url: observedUrl, options: observedOptions }),
    setNow: (value: number) => {
      now = value;
    },
  };
};

describe("Sarah LiveKit acceptance control channel", () => {
  test("rejects a non-production gateway or malformed ticket before opening a socket", () => {
    const factory = vi.fn<SarahLiveKitAcceptanceControlSocketFactory>();
    expect(() =>
      openSarahLiveKitAcceptanceControlChannel(
        {
          gatewayUrl: "wss://openagents.com/api/omega/sarah/voice/connect?ticket=private",
          ticket: "acceptance-ticket-must-stay-private",
          identity,
          disclosureRef: "disclosure.sarah_livekit_acceptance.v1",
        },
        () => 100,
        factory,
      ),
    ).toThrow("control grant is invalid");
    expect(() =>
      openSarahLiveKitAcceptanceControlChannel(
        {
          gatewayUrl: "wss://openagents.com/api/omega/sarah/voice/connect",
          ticket: "short",
          identity,
          disclosureRef: "disclosure.sarah_livekit_acceptance.v1",
        },
        () => 100,
        factory,
      ),
    ).toThrow("control grant is invalid");
    expect(factory).not.toHaveBeenCalled();
  });

  test("authenticates, sequences hello-interrupt-close, and requires both interrupt signals", async () => {
    const { channel, socket, observed, setNow } = setup();
    socket.open();

    expect(observed().url).toBe("wss://openagents.com/api/omega/sarah/voice/connect");
    expect(observed().options?.headers).toMatchObject({
      "x-openagents-sarah-voice-session": identity.sessionRef,
      "x-openagents-sarah-voice-ticket": "acceptance-ticket-must-stay-private",
    });
    expect(JSON.parse(socket.sent[0] ?? "{}")).toEqual({
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      identity,
      sequence: 0,
      _tag: "session_hello",
      disclosureRef: "disclosure.sarah_livekit_acceptance.v1",
    });
    expect(socket.sent[0]).not.toContain("acceptance-ticket-must-stay-private");

    socket.message(serverControl(0, { _tag: "lifecycle", state: "connecting" }));
    socket.message(
      serverControl(1, {
        _tag: "session_ready",
        model: "gpt-realtime-2.1",
        expiresAtMs: 10_000,
        reservedCreditMsat: 1_000,
      }),
    );
    await expect(channel.ready).resolves.toBe(100);

    const interruption = channel.interrupt();
    expect(JSON.parse(socket.sent[1] ?? "{}")).toMatchObject({
      sequence: 1,
      _tag: "interrupt",
    });
    setNow(120);
    socket.message(serverControl(2, { _tag: "interrupt_ack" }));
    expect(await Promise.race([interruption, Promise.resolve("pending")])).toBe("pending");
    setNow(125);
    socket.message(serverControl(3, { _tag: "lifecycle", state: "interrupted" }));
    await expect(interruption).resolves.toEqual({
      acknowledgedAtMs: 120,
      interruptedAtMs: 125,
    });

    const closed = channel.close();
    expect(JSON.parse(socket.sent[2] ?? "{}")).toMatchObject({
      sequence: 2,
      _tag: "close",
      reason: "user_stop",
    });
    socket.closed();
    await expect(closed).resolves.toBeUndefined();
    expect(socket.terminate).not.toHaveBeenCalled();
  });

  test("fails closed on binary media without exposing the ticket", async () => {
    const { channel, socket } = setup();
    const ready = channel.ready;
    socket.open();
    socket.message({ ignored: true }, true);

    const error = await ready.catch((cause: unknown) => cause);
    expect(error).toBeInstanceOf(Error);
    expect((error as Error).message).toBe(
      "Sarah LiveKit acceptance control channel received binary media",
    );
    expect((error as Error).message).not.toContain("acceptance-ticket-must-stay-private");
    expect(socket.terminate).toHaveBeenCalledOnce();
  });

  test("rejects server identity or sequence drift and early transport failure", async () => {
    const first = setup();
    const firstReady = first.channel.ready;
    first.socket.open();
    first.socket.message(
      serverControl(1, {
        _tag: "session_ready",
        model: "gpt-realtime-2.1",
        expiresAtMs: 10_000,
        reservedCreditMsat: 1_000,
      }),
    );
    await expect(firstReady).rejects.toThrow("authority or sequence disagreed");

    const second = setup();
    const secondReady = second.channel.ready;
    second.socket.open();
    second.socket.error();
    await expect(secondReady).rejects.toThrow("control transport failed");
    expect(second.socket.terminate).toHaveBeenCalledOnce();
  });
});
