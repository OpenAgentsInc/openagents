import { useCallback, useEffect, useMemo, useState } from "react";
import { Camera, CameraView } from "expo-camera";
import { randomUUID } from "expo-crypto";
import { Effect } from "effect";

import {
  createOmegaDeviceBridgeClient,
  decodeOmegaBridgePairingBootstrap,
  openExpoOmegaDeviceBridgeStore,
  type OmegaBridgePairingBootstrap,
  type OmegaDeviceBridgeClient,
  type OmegaDeviceBridgeState,
  type OmegaDeviceBridgeWebSocket,
  type OmegaMirrorThread,
} from "../workroom/omega-device-bridge-client";
import { openExpoIssue31DeviceIdentity } from "../workroom/issue31-device-key-vault";
import { productSafeNotice, startOmegaBridgeSession } from "./omega-bridge-session";
import {
  connectionToneOf,
  OmegaHomeView,
  type OmegaHomeActivity,
  type OmegaHomeViewModel,
} from "./omega-home-view";

const PAIRING_SCAN_TIMEOUT_MS = 120_000;
const DEFAULT_BRIDGE_PORT = 4_317;

const offlineState: OmegaDeviceBridgeState = {
  paired: false,
  connection: {
    state: "offline",
    endpoint: null,
    heartbeatAt: null,
    relayObservedAt: null,
    staleSince: null,
  },
  mirror: null,
  recovery: "none",
  refusal: null,
};

/** Scan the code the desktop shows in its sidebar. */
export const scanOmegaDesktopPairingQr = async (): Promise<OmegaBridgePairingBootstrap | null> => {
  const permission = await Camera.requestCameraPermissionsAsync();
  if (!permission.granted) {
    throw new Error("Camera access is required to scan the desktop pairing code.");
  }
  if (!CameraView.isModernBarcodeScannerAvailable) {
    throw new Error("The system QR scanner is unavailable on this device.");
  }
  return new Promise<OmegaBridgePairingBootstrap | null>((resolve, reject) => {
    let settled = false;
    const finish = (result: OmegaBridgePairingBootstrap | Error): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      subscription.remove();
      if (result instanceof Error) reject(result);
      else resolve(result);
    };
    const subscription = CameraView.onModernBarcodeScanned((event) => {
      try {
        const pairing = decodeOmegaBridgePairingBootstrap(JSON.parse(event.data) as unknown);
        finish(pairing);
        void CameraView.dismissScanner().catch(() => undefined);
      } catch {
        finish(new Error("The desktop pairing code is invalid."));
      }
    });
    const timeout = setTimeout(
      () => finish(new Error("The desktop pairing scan timed out.")),
      PAIRING_SCAN_TIMEOUT_MS,
    );
    void CameraView.launchScanner({ barcodeTypes: ["qr"] }).catch(() =>
      finish(new Error("The desktop pairing scan was cancelled.")),
    );
  });
};

const openDefaultBridge = async (): Promise<OmegaDeviceBridgeClient> => {
  const identity = await openExpoIssue31DeviceIdentity();
  return createOmegaDeviceBridgeClient({
    identity,
    store: openExpoOmegaDeviceBridgeStore(),
    createSocket: (url) => new WebSocket(url) as unknown as OmegaDeviceBridgeWebSocket,
    now: Date.now,
    randomNonce: randomUUID,
    defaultPort: DEFAULT_BRIDGE_PORT,
  });
};

/**
 * A development-only pairing source. A simulator has no camera, so the QR rung
 * cannot complete there. Absent unless its variable is set, and it decodes
 * through the same schema the scanner uses.
 */
const developmentPairing = async (): Promise<OmegaBridgePairingBootstrap | null> => {
  const inline = process.env.EXPO_PUBLIC_OMEGA_PAIRING_BOOTSTRAP?.trim();
  if (inline !== undefined && inline !== "") {
    try {
      return decodeOmegaBridgePairingBootstrap(JSON.parse(inline) as unknown);
    } catch {
      return null;
    }
  }
  const url = process.env.EXPO_PUBLIC_OMEGA_PAIRING_BOOTSTRAP_URL?.trim();
  if (url === undefined || url === "") return null;
  try {
    const response = await fetch(url);
    if (!response.ok) return null;
    return decodeOmegaBridgePairingBootstrap((await response.json()) as unknown);
  } catch {
    return null;
  }
};

const connectionLabelOf = (state: OmegaDeviceBridgeState): string => {
  switch (state.connection.state) {
    case "direct":
      return "Direct";
    case "relay":
      return "Relay";
    case "offline":
      return "Offline";
  }
};

const stalenessOf = (state: OmegaDeviceBridgeState, observedAt: number): string => {
  if (state.connection.state === "direct") return "Live from your desktop";
  const staleSince = state.connection.staleSince ?? state.mirror?.projectedAt ?? null;
  if (staleSince === null) {
    return state.connection.state === "relay"
      ? "Relay is available. The desktop mirror is not current."
      : "The desktop is unreachable.";
  }
  const seconds = Math.max(0, Math.floor((observedAt - staleSince) / 1_000));
  if (seconds < 60) return `Last desktop update ${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `Last desktop update ${minutes}m ago`;
  return `Last desktop update ${Math.floor(minutes / 60)}h ago`;
};

const activityOf = (state: OmegaDeviceBridgeState): ReadonlyArray<OmegaHomeActivity> =>
  [
    ...(state.mirror?.threads.map(
      (thread): OmegaHomeActivity => ({ type: "thread", updatedAt: thread.updatedAt, thread }),
    ) ?? []),
    ...(state.mirror?.runs.map(
      (run): OmegaHomeActivity => ({ type: "run", updatedAt: run.updatedAt, run }),
    ) ?? []),
    // `toSorted` is ES2023 and Hermes does not implement it.
  ].sort((left, right) => right.updatedAt - left.updatedAt);

export const OmegaHomeScreen = ({
  bridge,
  scanPairing = scanOmegaDesktopPairingQr,
}: {
  readonly bridge?: OmegaDeviceBridgeClient;
  readonly scanPairing?: () => Promise<OmegaBridgePairingBootstrap | null>;
}) => {
  const [client, setClient] = useState<OmegaDeviceBridgeClient | null>(bridge ?? null);
  const [state, setState] = useState<OmegaDeviceBridgeState>(bridge?.state() ?? offlineState);
  const [selectedThreadRef, setSelectedThreadRef] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [observedAt, setObservedAt] = useState(Date.now());

  // Relative stamps have to age on their own. Without a tick, a row that says
  // "2m" keeps saying "2m" until the desktop happens to send something.
  useEffect(() => {
    const tick = setInterval(() => setObservedAt(Date.now()), 30_000);
    return () => clearInterval(tick);
  }, []);

  useEffect(
    () =>
      startOmegaBridgeSession({
        bridge,
        openBridge: openDefaultBridge,
        pairing: developmentPairing,
        onClient: setClient,
        onState: (next) => {
          setState(next);
          setObservedAt(Date.now());
        },
        onNotice: setNotice,
      }),
    [bridge],
  );

  const selectedThread = useMemo<OmegaMirrorThread | null>(() => {
    if (selectedThreadRef === null) return null;
    return state.mirror?.threads.find((thread) => thread.threadRef === selectedThreadRef) ?? null;
  }, [selectedThreadRef, state.mirror]);

  const onPairPressed = useCallback(() => {
    void (async () => {
      if (client === null) return;
      try {
        const pairing = await scanPairing();
        if (pairing === null) return;
        setNotice(null);
        await Effect.runPromise(
          client.connect({ announcements: [], pairing, manualMagicDns: null }).pipe(
            Effect.catch((error) => Effect.sync(() => setNotice(productSafeNotice(error.message)))),
          ),
        );
      } catch (error: unknown) {
        setNotice(
          error instanceof Error ? productSafeNotice(error.message) : "The desktop pairing failed.",
        );
      }
    })();
  }, [client, scanPairing]);

  const model: OmegaHomeViewModel = {
    desktopName: state.mirror?.desktopName ?? "Omega desktop",
    connectionLabel: connectionLabelOf(state),
    connectionTone: connectionToneOf(state),
    staleness: stalenessOf(state, observedAt),
    paired: state.paired,
    notice,
    activity: activityOf(state),
    selectedThread,
    threadDraft: draft,
    // The device bridge is read-only by law (OMEGA-DELTA-0154). Sending from
    // the phone travels the signed relay command lane, which this surface does
    // not open yet, so it says so rather than offering a control that cannot work.
    commandLaneAvailable: false,
    commandNotice: null,
    now: observedAt,
  };

  return (
    <OmegaHomeView
      model={model}
      actions={{
        onPairPressed,
        onActivitySelected: setSelectedThreadRef,
        onThreadClosed: () => setSelectedThreadRef(null),
        onDraftChanged: setDraft,
        onEnqueuePressed: () => undefined,
        onSteerPressed: () => undefined,
      }}
    />
  );
};
