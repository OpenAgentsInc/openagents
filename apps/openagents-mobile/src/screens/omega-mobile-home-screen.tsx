import { useEffect, useMemo, useState } from "react";
import { Camera, CameraView } from "expo-camera";
import { randomUUID } from "expo-crypto";
import { Platform, View as RNView } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { khalaTheme } from "@effect-native/tokens";
import { Effect, Stream } from "@effect-native/core/effect";

import { EffectNativeHost } from "../effect-native/effect-native-host";
import {
  createOmegaDeviceBridgeClient,
  decodeOmegaBridgePairingBootstrap,
  openExpoOmegaDeviceBridgeStore,
  type OmegaBridgePairingBootstrap,
  type OmegaDeviceBridgeClient,
  type OmegaDeviceBridgeWebSocket,
} from "../workroom/omega-device-bridge-client";
import { openExpoIssue31DeviceIdentity } from "../workroom/issue31-device-key-vault";
import {
  buildOmegaMobileHomeProgram,
  renderOmegaMobileHome,
  type OmegaMobileHomeConnectRequest,
  type OmegaMobileHomeProgram,
  type OmegaMobileHomeState,
} from "./omega-mobile-home";

const enPlatform = Platform.OS === "android" ? ("android" as const) : ("ios" as const);

const pairingScanTimeoutMs = 120_000;

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
    const finish = (
      result:
        | Readonly<{ type: "success"; pairing: OmegaBridgePairingBootstrap }>
        | Readonly<{ type: "failure"; error: Error }>,
    ): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      subscription.remove();
      if (result.type === "success") resolve(result.pairing);
      else reject(result.error);
    };
    const subscription = CameraView.onModernBarcodeScanned((event) => {
      void Promise.resolve()
        .then(() => decodeOmegaBridgePairingBootstrap(JSON.parse(event.data) as unknown))
        .then((pairing) => {
          finish({ type: "success", pairing });
          void CameraView.dismissScanner().catch((error: unknown) => {
            console.error("Failed to dismiss the Omega pairing scanner", error);
          });
        })
        .catch((error: unknown) => {
          finish({
            type: "failure",
            error:
              error instanceof Error ? error : new Error("The desktop pairing code is invalid."),
          });
        });
    });
    const timeout = setTimeout(() => {
      finish({
        type: "failure",
        error: new Error("The desktop pairing scan timed out."),
      });
    }, pairingScanTimeoutMs);
    void CameraView.launchScanner({ barcodeTypes: ["qr"] }).catch((error: unknown) => {
      finish({
        type: "failure",
        error:
          error instanceof Error ? error : new Error("The desktop pairing scan was cancelled."),
      });
    });
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
    defaultPort: 4_317,
  });
};

const defaultConnectRequest = (): OmegaMobileHomeConnectRequest => ({
  announcements: [],
  pairing: null,
  manualMagicDns: process.env.EXPO_PUBLIC_OMEGA_MAGIC_DNS?.trim() || null,
});

const bootState = (): OmegaMobileHomeState => ({
  bridge: {
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
  },
  selectedThreadRef: null,
  observedAt: Date.now(),
  notice: null,
});

export const OmegaMobileHomeScreen = ({
  bridge,
  scanPairing = scanOmegaDesktopPairingQr,
  connectRequest,
}: {
  readonly bridge?: OmegaDeviceBridgeClient;
  readonly scanPairing?: () => Promise<OmegaBridgePairingBootstrap | null>;
  readonly connectRequest?: OmegaMobileHomeConnectRequest;
}) => {
  const [program, setProgram] = useState<OmegaMobileHomeProgram | null>(null);
  const [initialState, setInitialState] = useState<OmegaMobileHomeState>(bootState);
  const resolvedConnectRequest = useMemo(
    () => connectRequest ?? defaultConnectRequest(),
    [connectRequest],
  );
  const fallbackView = useMemo(() => renderOmegaMobileHome(initialState), [initialState]);
  const fallbackStream = useMemo(() => Stream.make(fallbackView), [fallbackView]);

  useEffect(() => {
    let active = true;
    let mountedProgram: OmegaMobileHomeProgram | null = null;
    void (bridge === undefined ? openDefaultBridge() : Promise.resolve(bridge))
      .then((client) => {
        const nextProgram = buildOmegaMobileHomeProgram({
          bridge: client,
          connectRequest: resolvedConnectRequest,
          scanPairing,
        });
        if (!active) {
          return nextProgram.close();
        }
        mountedProgram = nextProgram;
        setProgram(nextProgram);
      })
      .catch((error: unknown) => {
        if (!active) return;
        setInitialState((current) => ({
          ...current,
          notice:
            error instanceof Error ? error.message : "The Omega device bridge is unavailable.",
        }));
        setProgram(null);
      });
    return () => {
      active = false;
      if (mountedProgram !== null) {
        void mountedProgram.close().catch((error: unknown) => {
          console.error("Failed to close the Omega mobile home program", error);
        });
      }
    };
  }, [bridge, resolvedConnectRequest, scanPairing]);

  return (
    <RNView style={{ flex: 1, backgroundColor: khalaTheme.color.background }}>
      <SafeAreaView edges={["top", "bottom"]} style={{ flex: 1 }}>
        {program === null ? (
          <EffectNativeHost
            viewStream={fallbackStream}
            report={() => Effect.void}
            theme={khalaTheme}
            platform={enPlatform}
            initialView={fallbackView}
          />
        ) : (
          <EffectNativeHost
            viewStream={program.viewStream}
            report={program.report}
            theme={khalaTheme}
            platform={enPlatform}
            initialView={renderOmegaMobileHome(program.initialState)}
          />
        )}
      </SafeAreaView>
    </RNView>
  );
};
