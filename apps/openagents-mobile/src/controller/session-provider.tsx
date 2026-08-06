import type { EnqueueInput, TransportResult } from "@openagentsinc/client-command-outbox";
import { ConvexProvider, ConvexReactClient } from "convex/react";
import * as SecureStore from "expo-secure-store";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import {
  clearNativeSessionCredential,
  loadNativeSessionCredential,
  saveNativeSessionCredential,
  type NativeSessionCredential,
  type NativeSessionSecureStore,
} from "../auth/native-session-vault";
import { useMobileClientOutbox } from "../outbox/client-outbox-provider";
import {
  ControllerApiError,
  fetchControllerBootstrap,
  makeControllerTransport,
  sendImmediateInterrupt,
} from "./api";
import type { ControllerBootstrap, ControllerTarget } from "./contracts";

export type ControllerSessionState =
  | Readonly<{ phase: "initializing"; bootstrap: null; message: null }>
  | Readonly<{ phase: "signed_out"; bootstrap: null; message: null }>
  | Readonly<{ phase: "ready"; bootstrap: ControllerBootstrap; message: null }>
  | Readonly<{ phase: "failed"; bootstrap: null; message: string }>;

type ControllerSessionValue = ControllerSessionState &
  Readonly<{
    acceptCredential: (credential: NativeSessionCredential) => Promise<void>;
    signOut: () => Promise<void>;
    enqueueAndDrain: (
      command: EnqueueInput,
      gate: Readonly<{
        shellLive: boolean;
        decisionRevisions: Readonly<Record<string, string>>;
      }>,
      options?: Readonly<{ clearObservationKey?: string }>,
    ) => Promise<Readonly<{ delivered: number; terminal: number; pending: number }>>;
    interrupt: (commandId: string, target: ControllerTarget) => Promise<TransportResult>;
  }>;

const initialState: ControllerSessionState = {
  phase: "initializing",
  bootstrap: null,
  message: null,
};

const ControllerSessionContext = createContext<ControllerSessionValue | null>(null);

const secureStore: NativeSessionSecureStore = {
  AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: SecureStore.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
  deleteItemAsync: (key, options) =>
    SecureStore.deleteItemAsync(key, options as SecureStore.SecureStoreOptions),
  getItemAsync: (key, options) =>
    SecureStore.getItemAsync(key, options as SecureStore.SecureStoreOptions),
  setItemAsync: (key, value, options) =>
    SecureStore.setItemAsync(key, value, options as SecureStore.SecureStoreOptions),
};

export const ControllerSessionProvider = ({ children }: { readonly children: ReactNode }) => {
  const outbox = useMobileClientOutbox();
  const [state, setState] = useState<ControllerSessionState>(initialState);
  const [client, setClient] = useState<ConvexReactClient | null>(null);
  const credentialRef = useRef<NativeSessionCredential | null>(null);
  const clientRef = useRef<ConvexReactClient | null>(null);

  const disconnect = useCallback(() => {
    clientRef.current?.clearAuth();
    void clientRef.current?.close();
    clientRef.current = null;
    setClient(null);
  }, []);

  const connect = useCallback(async (credential: NativeSessionCredential) => {
    const initial = await fetchControllerBootstrap({ credential, secureStore });
    credentialRef.current = initial.credential;
    const nextClient = new ConvexReactClient(initial.bootstrap.convexUrl, {
      unsavedChangesWarning: false,
    });
    nextClient.setAuth(async () => {
      const current = credentialRef.current;
      if (current === null) return null;
      try {
        const refreshed = await fetchControllerBootstrap({ credential: current, secureStore });
        credentialRef.current = refreshed.credential;
        return refreshed.bootstrap.token;
      } catch (error) {
        if (error instanceof ControllerApiError && error.reason === "signed_out") {
          credentialRef.current = null;
          await clearNativeSessionCredential(secureStore).catch(() => undefined);
          setState({ phase: "signed_out", bootstrap: null, message: null });
        }
        return null;
      }
    });
    clientRef.current = nextClient;
    setClient(nextClient);
    setState({ phase: "ready", bootstrap: initial.bootstrap, message: null });
  }, []);

  useEffect(() => {
    let active = true;
    void loadNativeSessionCredential(secureStore)
      .then(async (credential) => {
        if (!active) return;
        if (credential === null) {
          setState({ phase: "signed_out", bootstrap: null, message: null });
          return;
        }
        await connect(credential);
      })
      .catch((error: unknown) => {
        if (!active) return;
        const signedOut = error instanceof ControllerApiError && error.reason === "signed_out";
        setState(
          signedOut
            ? { phase: "signed_out", bootstrap: null, message: null }
            : {
                phase: "failed",
                bootstrap: null,
                message: error instanceof Error ? error.message : "The controller could not start.",
              },
        );
      });
    return () => {
      active = false;
      disconnect();
    };
  }, [connect, disconnect]);

  const acceptCredential = useCallback(
    async (credential: NativeSessionCredential) => {
      disconnect();
      await saveNativeSessionCredential(secureStore, credential);
      setState(initialState);
      await connect(credential);
    },
    [connect, disconnect],
  );

  const signOut = useCallback(async () => {
    disconnect();
    credentialRef.current = null;
    await clearNativeSessionCredential(secureStore);
    setState({ phase: "signed_out", bootstrap: null, message: null });
  }, [disconnect]);

  const updateCredential = useCallback((credential: NativeSessionCredential) => {
    credentialRef.current = credential;
  }, []);

  const enqueueAndDrain = useCallback(
    async (
      command: EnqueueInput,
      gate: Readonly<{
        shellLive: boolean;
        decisionRevisions: Readonly<Record<string, string>>;
      }>,
      options?: Readonly<{ clearObservationKey?: string }>,
    ) => {
      if (outbox.phase !== "ready") throw new Error("The command outbox is not ready.");
      const credential = credentialRef.current;
      if (credential === null) throw new Error("Sign in before sending a command.");
      if (options?.clearObservationKey === undefined) {
        await outbox.runtime.enqueue(command);
      } else {
        await outbox.runtime.enqueueAndClearObservation(command, options.clearObservationKey);
      }
      return await outbox.runtime.drain(
        makeControllerTransport({
          credential: () => {
            const current = credentialRef.current;
            if (current === null) throw new Error("The mobile session ended.");
            return current;
          },
          updateCredential,
          secureStore,
        }),
        {
          convexConnected: clientRef.current !== null,
          shellLive: gate.shellLive,
          decisionRevisions: gate.decisionRevisions,
        },
      );
    },
    [outbox, updateCredential],
  );

  const interrupt = useCallback(
    async (commandId: string, target: ControllerTarget) => {
      const credential = credentialRef.current;
      if (credential === null || clientRef.current === null) {
        throw new Error("Interrupt is unavailable while offline.");
      }
      if (outbox.phase !== "ready") throw new Error("The command policy is not ready.");
      outbox.runtime.authorizeImmediate({ operation: "runtime.interrupt", online: true });
      return await sendImmediateInterrupt({
        commandId,
        target,
        credential,
        updateCredential,
        secureStore,
      });
    },
    [outbox, updateCredential],
  );

  const value = useMemo<ControllerSessionValue>(
    () => ({ ...state, acceptCredential, signOut, enqueueAndDrain, interrupt }),
    [acceptCredential, enqueueAndDrain, interrupt, signOut, state],
  );

  const content = (
    <ControllerSessionContext.Provider value={value}>{children}</ControllerSessionContext.Provider>
  );
  return client === null ? content : <ConvexProvider client={client}>{content}</ConvexProvider>;
};

export const useControllerSession = (): ControllerSessionValue => {
  const value = useContext(ControllerSessionContext);
  if (value === null) throw new Error("ControllerSessionProvider is missing.");
  return value;
};
