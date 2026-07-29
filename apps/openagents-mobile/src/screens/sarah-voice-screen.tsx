import { useCallback, useEffect, useRef, useState } from "react";
import {
  AppState,
  FlatList,
  Platform,
  View,
  type AppStateStatus,
  type TextStyle,
  type ViewStyle,
} from "react-native";
import { requestRecordingPermissionsAsync, setAudioModeAsync, useAudioStream } from "expo-audio";
import { CryptoDigestAlgorithm, digest, randomUUID } from "expo-crypto";
import * as SecureStore from "expo-secure-store";

import RealtimeAudio from "../../modules/expo-realtime-audio";
import type { NativeSessionSecureStore } from "../auth/native-session-vault";
import {
  SarahVoiceClient,
  type SarahVoiceSnapshot,
  type SarahVoiceSocket,
} from "../sarah-voice/client";
import { sarahVoiceAppStateAction } from "../sarah-voice/app-state";
import { makeSarahVoiceDeviceLinkRecovery } from "../sarah-voice/device-link";
import { SARAH_BETA_VOICE_BASE_URL } from "../sarah-voice/environment";
import { bytesToBase64 } from "../sarah-voice/protocol";
import { makeSarahVoiceSessionVault } from "../sarah-voice/session-vault";
import {
  expoIssue31DeviceKeyPlatform,
  openExpoIssue31DeviceIdentity,
  SARAH_STAGING_DEVICE_KEY_STORE_KEY,
  type Issue31DeviceIdentity,
  type Issue31SecureStore,
} from "../workroom/issue31-device-key-vault";
import { Button } from "../ui/button";
import { Screen } from "../ui/screen";
import { Badge, Card, Divider, EmptyState } from "../ui/surfaces";
import { Text } from "../ui/text";
import { colors, radius, spacing } from "../ui/theme";

const SARAH_DEVICE_KEY_STORE = SARAH_STAGING_DEVICE_KEY_STORE_KEY;
const PLAYBACK_DRAIN_POLL_MS = 40;
const unsupportedMicrophoneFormat = "unsupported_microphone_format";

const microphoneStartError = (error: unknown): string =>
  error instanceof Error && error.message === unsupportedMicrophoneFormat
    ? "This device cannot provide the 24 kHz mono microphone format that Sarah voice requires."
    : "The microphone could not start.";

type NativeWebSocketConstructor = new (
  url: string,
  protocols: ReadonlyArray<string> | null,
  options: Readonly<{ headers: Readonly<Record<string, string>> }>,
) => SarahVoiceSocket;

const createSocket = (url: string, headers: Readonly<Record<string, string>>): SarahVoiceSocket =>
  new (WebSocket as unknown as NativeWebSocketConstructor)(url, null, {
    headers,
  });

const phaseLabel = (snapshot: SarahVoiceSnapshot): string => {
  if (snapshot.muted && snapshot.phase === "listening") return "Muted";
  switch (snapshot.phase) {
    case "idle":
      return "Ready";
    case "connecting":
      return "Connecting";
    case "listening":
      return "Listening";
    case "thinking":
      return "Thinking";
    case "speaking":
      return "Speaking";
    case "interrupted":
      return "Interrupted";
    case "reconnecting":
      return "Reconnecting";
    case "ended":
      return "Ended";
    case "error":
      return "Unavailable";
  }
};

const phaseTone = (snapshot: SarahVoiceSnapshot): "success" | "info" | "danger" | "neutral" =>
  snapshot.phase === "error"
    ? "danger"
    : snapshot.phase === "listening" || snapshot.phase === "speaking"
      ? "success"
      : snapshot.phase === "connecting" ||
          snapshot.phase === "thinking" ||
          snapshot.phase === "reconnecting"
        ? "info"
        : "neutral";

export const SarahVoiceScreen = ({ onClose }: { readonly onClose: () => void }) => {
  const [snapshot, setSnapshot] = useState<SarahVoiceSnapshot>({
    phase: "idle",
    muted: false,
    message: null,
    retryable: false,
    transcripts: [],
    reservedCreditMsat: null,
  });
  const [localError, setLocalError] = useState<string | null>(null);
  const [clientReady, setClientReady] = useState(false);
  const clientRef = useRef<SarahVoiceClient | null>(null);
  const identityRef = useRef<Issue31DeviceIdentity | null>(null);
  const playbackDrainTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  const { stream, isStreaming } = useAudioStream({
    sampleRate: 24_000,
    channels: 1,
    encoding: "int16",
    onBuffer: (buffer) => {
      clientRef.current?.sendAudio(new Uint8Array(buffer.data), buffer.sampleRate, buffer.channels);
    },
  });

  const stopCapture = useCallback((): void => {
    try {
      if (stream.isStreaming) stream.stop();
    } catch {
      // Expo can dispose its native shared object before React runs this cleanup.
    }
  }, [stream]);

  const startCapture = useCallback(async (): Promise<void> => {
    if (stream.isStreaming) return;
    RealtimeAudio.stop();
    await setAudioModeAsync({
      allowsRecording: true,
      allowsBackgroundRecording: false,
      interruptionMode: "doNotMix",
      playsInSilentMode: true,
      shouldPlayInBackground: false,
      shouldRouteThroughEarpiece: false,
    });
    await stream.start();
    if (stream.sampleRate !== 24_000 || stream.channels !== 1) {
      stream.stop();
      throw new Error(unsupportedMicrophoneFormat);
    }
  }, [stream]);

  const handleCaptureFailure = useCallback(
    (error: unknown): void => {
      stopCapture();
      setLocalError(microphoneStartError(error));
      void clientRef.current?.end("transport_error");
    },
    [stopCapture],
  );

  const clearDrainTimer = useCallback((): void => {
    if (playbackDrainTimer.current !== null) {
      clearInterval(playbackDrainTimer.current);
      playbackDrainTimer.current = null;
    }
  }, []);

  const waitForPlaybackDrain = useCallback((): void => {
    clearDrainTimer();
    playbackDrainTimer.current = setInterval(() => {
      const status = RealtimeAudio.getStatus();
      if (status.interrupted) {
        clearDrainTimer();
        RealtimeAudio.stop();
        setLocalError("Audio playback was interrupted. Retry Sarah voice when audio is ready.");
        void clientRef.current?.end("transport_error");
        return;
      }
      if (status.queuedFrames > 0) return;
      clearDrainTimer();
      RealtimeAudio.stop();
      const current = clientRef.current?.snapshot();
      if (current?.phase === "listening" && !current.muted && AppState.currentState === "active") {
        void startCapture().catch(handleCaptureFailure);
      }
    }, PLAYBACK_DRAIN_POLL_MS);
  }, [clearDrainTimer, handleCaptureFailure, startCapture]);

  useEffect(() => {
    let active = true;
    let unsubscribeState: (() => void) | undefined;
    let unsubscribeAudio: (() => void) | undefined;

    void (async () => {
      try {
        const identity = await openExpoIssue31DeviceIdentity(SARAH_DEVICE_KEY_STORE);
        if (!active) {
          identity.close();
          return;
        }
        if (SARAH_DEVICE_KEY_STORE !== undefined) {
          console.info("Sarah staging identity public key", identity.publicKeyHex);
        }
        identityRef.current = identity;
        const client = new SarahVoiceClient({
          baseUrl: SARAH_BETA_VOICE_BASE_URL,
          publicKeyHex: identity.publicKeyHex,
          signer: identity.signer,
          vault: makeSarahVoiceSessionVault(
            SecureStore as unknown as Issue31SecureStore,
            expoIssue31DeviceKeyPlatform(),
          ),
          fetch: globalThis.fetch,
          createSocket,
          sha256: async (bytes) =>
            new Uint8Array(await digest(CryptoDigestAlgorithm.SHA256, Uint8Array.from(bytes))),
          randomUuid: randomUUID,
          now: Date.now,
          setTimeout,
          clearTimeout,
          recoverDeviceLink: makeSarahVoiceDeviceLinkRecovery(
            SecureStore as unknown as NativeSessionSecureStore,
          ),
        });
        clientRef.current = client;
        setClientReady(true);
        unsubscribeState = client.subscribe(setSnapshot);
        unsubscribeAudio = client.onAudio(({ pcm }) => {
          if (AppState.currentState !== "active") return;
          if (RealtimeAudio.getStatus().interrupted) {
            RealtimeAudio.stop();
            setLocalError("Audio playback was interrupted. Retry Sarah voice when audio is ready.");
            void client.end("transport_error");
            return;
          }
          stopCapture();
          clearDrainTimer();
          try {
            RealtimeAudio.start(24_000);
            RealtimeAudio.enqueue(bytesToBase64(pcm));
          } catch {
            const played = RealtimeAudio.getStatus().playedMilliseconds;
            RealtimeAudio.stop();
            client.interrupt(played);
            setLocalError("Sarah audio could not keep up. Playback was interrupted safely.");
          }
        });
      } catch {
        if (active) setLocalError("Sarah voice could not start securely. Try again.");
      }
    })();

    const appState = AppState.addEventListener("change", (next: AppStateStatus) => {
      const action = sarahVoiceAppStateAction(next);
      if (action === "end_session") {
        clientRef.current?.setForeground(false);
        stopCapture();
        clearDrainTimer();
        RealtimeAudio.stop();
        return;
      }
      if (action === "resume") {
        clientRef.current?.setForeground(true);
        return;
      }
      stopCapture();
      clearDrainTimer();
      RealtimeAudio.stop();
    });

    return () => {
      active = false;
      appState.remove();
      unsubscribeState?.();
      unsubscribeAudio?.();
      stopCapture();
      clearDrainTimer();
      RealtimeAudio.stop();
      void clientRef.current?.end("app_backgrounded");
      clientRef.current = null;
      identityRef.current?.close();
      identityRef.current = null;
    };
  }, [clearDrainTimer, stopCapture]);

  useEffect(() => {
    if (snapshot.phase === "speaking") {
      stopCapture();
      return;
    }
    if (
      snapshot.phase === "connecting" ||
      snapshot.phase === "thinking" ||
      snapshot.phase === "reconnecting" ||
      snapshot.phase === "ended" ||
      snapshot.phase === "error"
    ) {
      stopCapture();
      if (snapshot.phase === "ended" || snapshot.phase === "error") {
        clearDrainTimer();
        RealtimeAudio.stop();
      }
      return;
    }
    if (snapshot.phase === "listening") {
      const playback = RealtimeAudio.getStatus();
      if (playback.started && playback.queuedFrames > 0) {
        waitForPlaybackDrain();
      } else if (!snapshot.muted && AppState.currentState === "active") {
        void startCapture().catch(handleCaptureFailure);
      }
    }
  }, [
    clearDrainTimer,
    handleCaptureFailure,
    snapshot.muted,
    snapshot.phase,
    startCapture,
    stopCapture,
    waitForPlaybackDrain,
  ]);

  const requireMicrophonePermission = useCallback(async (): Promise<boolean> => {
    setLocalError(null);
    const permission = await requestRecordingPermissionsAsync();
    if (permission.granted) return true;
    setLocalError(
      permission.canAskAgain
        ? "Microphone access is required for a live conversation with Sarah."
        : "Microphone access is off. Enable it for OpenAgents in system settings.",
    );
    return false;
  }, []);

  const start = useCallback(() => {
    void (async () => {
      if (!clientReady || !(await requireMicrophonePermission())) return;
      await clientRef.current?.start();
    })().catch(() => setLocalError("Sarah voice could not start."));
  }, [clientReady, requireMicrophonePermission]);

  const retry = useCallback(() => {
    void (async () => {
      if (!clientReady || !(await requireMicrophonePermission())) return;
      await clientRef.current?.retry();
    })().catch(() => setLocalError("Sarah voice could not retry."));
  }, [clientReady, requireMicrophonePermission]);

  const toggleMute = useCallback(() => {
    const next = !snapshot.muted;
    clientRef.current?.setMuted(next);
    if (next) {
      stopCapture();
    } else if (snapshot.phase === "listening") {
      void startCapture().catch(handleCaptureFailure);
    }
  }, [handleCaptureFailure, snapshot.muted, snapshot.phase, startCapture, stopCapture]);

  const interrupt = useCallback(() => {
    stopCapture();
    const played = RealtimeAudio.getStatus().playedMilliseconds;
    RealtimeAudio.flush();
    RealtimeAudio.stop();
    clientRef.current?.interrupt(played);
  }, [stopCapture]);

  const end = useCallback(() => {
    stopCapture();
    clearDrainTimer();
    RealtimeAudio.stop();
    void clientRef.current?.end("user_stop");
  }, [clearDrainTimer, stopCapture]);

  const active =
    snapshot.phase !== "idle" && snapshot.phase !== "ended" && snapshot.phase !== "error";
  const error = localError ?? snapshot.message;
  const shouldRetry = error !== null || (snapshot.phase === "error" && snapshot.retryable);

  return (
    <Screen>
      <View style={$header}>
        <Button label="← Omega" preset="ghost" onPress={onClose} style={$headerButton} />
        <Badge label={phaseLabel(snapshot)} tone={phaseTone(snapshot)} />
      </View>
      <View style={$intro}>
        <Text preset="display">Sarah voice</Text>
        <Text preset="body" color={colors.textDim}>
          Speak with Sarah through the managed OpenAgents voice service. The app connects securely
          to your OpenAgents account and never stores an OpenAI key.
        </Text>
        <Text preset="caption" color={colors.textFaint}>
          The microphone is active only while this screen says Listening. It stops while Sarah
          speaks and whenever the app leaves the foreground.
        </Text>
      </View>
      <Divider />
      <FlatList
        data={snapshot.transcripts}
        keyExtractor={(entry) => `${entry.source}:${entry.utteranceRef}`}
        renderItem={({ item }) => (
          <Card style={item.source === "user" ? $userTurn : $assistantTurn}>
            <Text preset="label" color={colors.textFaint}>
              {item.source === "user" ? "YOU" : "SARAH"}
              {item.final ? "" : " · LIVE"}
            </Text>
            <Text preset="body">{item.text}</Text>
          </Card>
        )}
        contentContainerStyle={$transcript}
        ListEmptyComponent={
          <EmptyState
            heading="Start when you are ready"
            body="Live transcript text appears here. Audio and transcript text are not stored by this screen."
          />
        }
        showsVerticalScrollIndicator={false}
      />
      {error === null ? null : (
        <View accessibilityRole="alert" style={$error}>
          <Text preset="body" color={colors.fault}>
            {error}
          </Text>
        </View>
      )}
      <View style={$controls}>
        {!active ? (
          <Button
            label={shouldRetry ? "Retry voice" : "Start voice"}
            onPress={shouldRetry ? retry : start}
            preset="primary"
            fullWidth
            disabled={!clientReady}
          />
        ) : (
          <>
            <Button
              label={snapshot.muted ? "Unmute" : "Mute"}
              onPress={toggleMute}
              preset="secondary"
            />
            <Button
              label="Interrupt"
              onPress={interrupt}
              preset="secondary"
              disabled={snapshot.phase !== "speaking"}
            />
            <Button label="End" onPress={end} preset="danger" />
          </>
        )}
      </View>
      <Text preset="caption" color={colors.textFaint} style={$captureState}>
        {isStreaming ? "Microphone capture active" : "Microphone capture off"}
      </Text>
    </Screen>
  );
};

const $header: ViewStyle = {
  minHeight: 44,
  paddingHorizontal: spacing.medium,
  paddingTop: spacing.extraSmall,
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
};
const $headerButton: ViewStyle = {
  minHeight: 0,
  paddingVertical: 0,
  paddingHorizontal: 0,
};
const $intro: ViewStyle = {
  paddingHorizontal: spacing.medium,
  paddingVertical: spacing.small,
  gap: spacing.extraSmall,
};
const $transcript: ViewStyle = {
  paddingHorizontal: spacing.medium,
  paddingVertical: spacing.small,
  gap: spacing.small,
};
const $userTurn: ViewStyle = { backgroundColor: colors.surfaceSunken };
const $assistantTurn: ViewStyle = {};
const $error: ViewStyle = {
  marginHorizontal: spacing.medium,
  padding: spacing.small,
  borderWidth: 1,
  borderColor: colors.fault,
  borderRadius: radius.medium,
  backgroundColor: colors.faultGlow,
};
const $controls: ViewStyle = {
  paddingHorizontal: spacing.medium,
  paddingTop: spacing.small,
  flexDirection: "row",
  flexWrap: "wrap",
  gap: spacing.extraSmall,
};
const $captureState: TextStyle = {
  paddingHorizontal: spacing.medium,
  paddingTop: spacing.extraSmall,
  paddingBottom: spacing.small,
};
