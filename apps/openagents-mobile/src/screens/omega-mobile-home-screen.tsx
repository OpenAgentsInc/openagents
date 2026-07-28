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
  issue31AdmittedHostPublicKeysFromEnvironment,
  issue31RelayUrlsFromEnvironment,
  openIssue31MobileNostrRuntime,
  type Issue31MobileNostrRuntime,
} from "../workroom/issue31-mobile-nostr-runtime";
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

/**
 * Development-only pairing input. A simulator has no camera, so the QR rung of
 * the dial ladder cannot complete there. This reads the same bootstrap the QR
 * carries from the environment so a simulator can exercise the handshake.
 * It is null in every build that does not set the variable, and it decodes
 * through the same schema the scanner uses, so an invalid bootstrap is refused
 * here exactly as it is refused there.
 */
const DEV_PAIRING_BOOTSTRAP_URL = process.env.EXPO_PUBLIC_OMEGA_PAIRING_BOOTSTRAP_URL?.trim();

const developmentPairingBootstrap = async (): Promise<OmegaBridgePairingBootstrap | null> => {
  const inline = process.env.EXPO_PUBLIC_OMEGA_PAIRING_BOOTSTRAP?.trim();
  if (inline !== undefined && inline !== "") {
    try {
      return decodeOmegaBridgePairingBootstrap(JSON.parse(inline) as unknown);
    } catch (error: unknown) {
      console.error("The development Omega pairing bootstrap is invalid", error);
      return null;
    }
  }
  if (DEV_PAIRING_BOOTSTRAP_URL === undefined || DEV_PAIRING_BOOTSTRAP_URL === "") return null;
  try {
    const response = await fetch(DEV_PAIRING_BOOTSTRAP_URL);
    if (!response.ok) return null;
    const body = (await response.json()) as unknown;
    const decoded = decodeOmegaBridgePairingBootstrap(body);
    return decoded;
  } catch (error: unknown) {
    console.error("The development Omega pairing bootstrap is unavailable", error);
    return null;
  }
};

const defaultConnectRequest = (
  pairing: OmegaBridgePairingBootstrap | null,
): OmegaMobileHomeConnectRequest => ({
  announcements: [],
  pairing,
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
  threadDraft: "",
  commandLaneAvailable: false,
  commandNotice: null,
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
  const fallbackView = useMemo(() => renderOmegaMobileHome(initialState), [initialState]);
  const fallbackStream = useMemo(() => Stream.make(fallbackView), [fallbackView]);

  useEffect(() => {
    let active = true;
    let mountedProgram: OmegaMobileHomeProgram | null = null;
    let commandRuntime: Issue31MobileNostrRuntime | null = null;
    void Promise.all([
      bridge === undefined ? openDefaultBridge() : Promise.resolve(bridge),
      openIssue31MobileNostrRuntime({
        relayUrls: issue31RelayUrlsFromEnvironment(),
        admittedHostPublicKeys: issue31AdmittedHostPublicKeysFromEnvironment(),
      }).catch((error: unknown) => {
        console.error("The signed Omega command lane is unavailable", error);
        return null;
      }),
      connectRequest === undefined
        ? developmentPairingBootstrap()
        : Promise.resolve<OmegaBridgePairingBootstrap | null>(null),
    ])
      .then(([client, runtime, devPairing]) => {
        commandRuntime = runtime;
        const nextProgram = buildOmegaMobileHomeProgram({
          bridge: client,
          connectRequest: connectRequest ?? defaultConnectRequest(devPairing),
          scanPairing,
          ...(runtime === null ? {} : { publishCommandIntent: runtime.publishCommandIntent }),
        });
        if (!active) {
          runtime?.close();
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
      commandRuntime?.close();
    };
  }, [bridge, connectRequest, scanPairing]);

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
