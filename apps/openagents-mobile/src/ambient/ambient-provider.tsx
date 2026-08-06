import { useQuery } from "convex/react";
import Constants from "expo-constants";
import * as Crypto from "expo-crypto";
import { Directory, File, Paths } from "expo-file-system";
import * as LiveActivity from "expo-live-activity";
import * as Linking from "expo-linking";
import * as Notifications from "expo-notifications";
import * as QuickActions from "expo-quick-actions";
import * as SecureStore from "expo-secure-store";
import { openDatabaseAsync, type SQLiteDatabase } from "expo-sqlite";
import { useShareIntentContext, type ShareIntent } from "expo-share-intent";
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
import { Platform } from "react-native";

import { decodeAttentionInbox } from "../controller/contracts";
import { attentionInboxQuery } from "../controller/convex-functions";
import { useControllerSession } from "../controller/session-provider";
import {
  AMBIENT_LIVE_ACTIVITY_VERSION,
  SHARE_INTAKE_VERSION,
  decodeLiveActivityShell,
  type ShareInboxItem,
} from "./contracts";
import { reconcileLiveActivity, type ReconciledLiveActivity } from "./live-activity";
import { watchAmbientNotificationResponses } from "./notification-ingress";
import {
  claimAmbientNotification,
  deleteShareInboxItem,
  initializeAmbientStore,
  listShareInboxItems,
  putShareInboxItem,
} from "./store";

type AmbientContextValue = Readonly<{
  phase: "initializing" | "ready" | "failed";
  items: ReadonlyArray<ShareInboxItem>;
  remove: (intakeId: string) => Promise<void>;
}>;

const AmbientContext = createContext<AmbientContextValue | null>(null);
const AMBIENT_DEVICE_ID_KEY = "openagents.ambient.device-id";

const ambientDeviceId = async (): Promise<string> => {
  const existing = await SecureStore.getItemAsync(AMBIENT_DEVICE_ID_KEY);
  if (existing !== null && existing.trim() !== "") return existing;
  const created = `device.${Crypto.randomUUID()}`;
  await SecureStore.setItemAsync(AMBIENT_DEVICE_ID_KEY, created, {
    keychainAccessible: SecureStore.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
  });
  return created;
};

const durableShareFile = async (
  intakeId: string,
  file: NonNullable<ShareIntent["files"]>[number],
): Promise<string> => {
  const directory = new Directory(Paths.document, "share-inbox");
  directory.create({ idempotent: true, intermediates: true });
  const extension = new File(file.path).extension.replace(/[^A-Za-z0-9.]/gu, "").slice(0, 16);
  const destination = new File(directory, `${intakeId}${extension}`);
  await new File(file.path).copy(destination, { overwrite: true });
  return destination.uri;
};

const persistShareIntent = async (database: SQLiteDatabase, intent: ShareIntent): Promise<void> => {
  const receivedAt = Date.now();
  const base = `share.${receivedAt}.${Crypto.randomUUID()}`;
  const candidates: Array<ShareInboxItem> = [];
  const text = intent.text?.trim();
  if (text) {
    candidates.push({
      version: SHARE_INTAKE_VERSION,
      intakeId: `${base}.text`,
      kind: "text",
      value: text.slice(0, 8_000),
      mimeType: "text/plain",
      receivedAt,
    });
  }
  const webUrl = intent.webUrl?.trim();
  if (webUrl) {
    candidates.push({
      version: SHARE_INTAKE_VERSION,
      intakeId: `${base}.url`,
      kind: "url",
      value: webUrl.slice(0, 8_000),
      mimeType: "text/uri-list",
      receivedAt,
    });
  }
  for (const [index, file] of (intent.files ?? []).entries()) {
    if (!file.mimeType.startsWith("image/")) continue;
    const intakeId = `${base}.image.${index}`;
    const value = await durableShareFile(intakeId, file);
    candidates.push({
      version: SHARE_INTAKE_VERSION,
      intakeId,
      kind: "image",
      value,
      mimeType: file.mimeType.slice(0, 160),
      receivedAt,
    });
  }
  for (const item of candidates) await putShareInboxItem(database, item);
};

const AmbientLiveActivityHost = () => {
  const session = useControllerSession();
  const raw = useQuery(attentionInboxQuery, { limit: 20 });
  const current = useRef<ReconciledLiveActivity>(null);

  useEffect(() => {
    if (session.phase !== "ready" || raw === undefined) return;
    const rows = decodeAttentionInbox(raw);
    const shell = rows.find((row) => row.attentionState !== "ready") ?? null;
    const projection =
      shell === null
        ? null
        : decodeLiveActivityShell({
            version: AMBIENT_LIVE_ACTIVITY_VERSION,
            workspaceId: session.bootstrap.workspace.workspaceId,
            aggregateType: shell.aggregateType,
            aggregateId: shell.aggregateId,
            attentionState: shell.attentionState,
            status: shell.status.slice(0, 160),
            generation: shell.generation,
            updatedAt: shell.updatedAt,
          });
    current.current = reconcileLiveActivity(
      {
        start: LiveActivity.startActivity,
        update: LiveActivity.updateActivity,
        stop: LiveActivity.stopActivity,
      },
      current.current,
      projection,
    );
  }, [raw, session]);

  useEffect(
    () => () => {
      current.current = reconcileLiveActivity(
        {
          start: LiveActivity.startActivity,
          update: LiveActivity.updateActivity,
          stop: LiveActivity.stopActivity,
        },
        current.current,
        null,
      );
    },
    [],
  );
  return null;
};

export const AmbientProvider = ({ children }: { readonly children: ReactNode }) => {
  const session = useControllerSession();
  const share = useShareIntentContext();
  const [database, setDatabase] = useState<SQLiteDatabase | null>(null);
  const [phase, setPhase] = useState<AmbientContextValue["phase"]>("initializing");
  const [items, setItems] = useState<ReadonlyArray<ShareInboxItem>>([]);

  const refresh = useCallback(async (nextDatabase: SQLiteDatabase) => {
    setItems(await listShareInboxItems(nextDatabase));
  }, []);

  useEffect(() => {
    let active = true;
    void openDatabaseAsync("openagents-ambient-v1.db")
      .then(async (nextDatabase) => {
        await initializeAmbientStore(nextDatabase);
        if (!active) return;
        setDatabase(nextDatabase);
        await refresh(nextDatabase);
        if (active) setPhase("ready");
      })
      .catch(() => {
        if (active) setPhase("failed");
      });
    return () => {
      active = false;
    };
  }, [refresh]);

  useEffect(() => {
    if (database === null || !share.hasShareIntent) return;
    void persistShareIntent(database, share.shareIntent)
      .then(() => refresh(database))
      .finally(() => share.resetShareIntent(true));
  }, [database, refresh, share]);

  useEffect(() => {
    if (database === null || session.phase !== "ready") return;
    return watchAmbientNotificationResponses({
      source: {
        defaultActionIdentifier: Notifications.DEFAULT_ACTION_IDENTIFIER,
        getLastResponse: Notifications.getLastNotificationResponseAsync,
        clearLastResponse: Notifications.clearLastNotificationResponseAsync,
        addResponseListener: Notifications.addNotificationResponseReceivedListener,
      },
      workspaceId: session.bootstrap.workspace.workspaceId,
      store: {
        claim: (notificationId) => claimAmbientNotification(database, notificationId),
      },
      openUrl: async (url) => {
        await Linking.openURL(url);
      },
    });
  }, [database, session]);

  useEffect(() => {
    const platform = Platform.OS === "ios" || Platform.OS === "android" ? Platform.OS : null;
    if (
      session.phase !== "ready" ||
      session.sessionKind === "screenshot_harness" ||
      platform === null
    ) {
      return;
    }
    const projectId = Constants.expoConfig?.extra?.openagents?.pushProjectId;
    if (typeof projectId !== "string" || projectId.trim() === "") return;
    let active = true;
    void Notifications.requestPermissionsAsync()
      .then(async (permission) => {
        if (!active || permission.status !== "granted") return;
        const [token, deviceId] = await Promise.all([
          Notifications.getExpoPushTokenAsync({ projectId }),
          ambientDeviceId(),
        ]);
        if (!active) return;
        await session.registerPushDevice({
          deviceId,
          pushToken: token.data,
          platform,
        });
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [session]);

  useEffect(() => {
    const openAction = (action: QuickActions.Action) => {
      const url = action.params?.url;
      if (typeof url === "string" && url.startsWith("openagents://")) {
        void Linking.openURL(url);
      }
    };
    void QuickActions.setItems([
      {
        id: "openagents.inbox",
        title: "Attention inbox",
        icon: "task",
        params: { url: "openagents://home" },
      },
      {
        id: "openagents.share-inbox",
        title: "Share inbox",
        icon: "share",
        params: { url: "openagents://intake" },
      },
      {
        id: "openagents.new-task",
        title: "New task",
        icon: "compose",
        params: { url: "openagents://new" },
      },
    ]);
    // The native module returns null when the app was not opened through a
    // shortcut even though its TypeScript surface currently says undefined.
    if (QuickActions.initial != null) openAction(QuickActions.initial);
    const subscription = QuickActions.addListener(openAction);
    return () => subscription.remove();
  }, []);

  const remove = useCallback(
    async (intakeId: string) => {
      if (database === null) return;
      await deleteShareInboxItem(database, intakeId);
      await refresh(database);
    },
    [database, refresh],
  );

  const value = useMemo(() => ({ phase, items, remove }), [items, phase, remove]);
  return (
    <AmbientContext.Provider value={value}>
      {children}
      {session.phase === "ready" ? <AmbientLiveActivityHost /> : null}
    </AmbientContext.Provider>
  );
};

export const useAmbient = (): AmbientContextValue => {
  const value = useContext(AmbientContext);
  if (value === null) throw new Error("AmbientProvider is missing.");
  return value;
};
