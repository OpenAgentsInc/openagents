import {
  decodeAmbientNotification,
  notificationDeepLink,
  type AmbientNotificationPayload,
} from "./contracts";

export type AmbientNotificationResponse = Readonly<{
  actionIdentifier: string;
  notification: Readonly<{
    request: Readonly<{
      content: Readonly<{ data?: unknown }>;
    }>;
  }>;
}>;

export interface AmbientNotificationSource {
  readonly defaultActionIdentifier: string;
  readonly getLastResponse: () => Promise<AmbientNotificationResponse | null>;
  readonly clearLastResponse: () => Promise<void>;
  readonly addResponseListener: (listener: (response: AmbientNotificationResponse) => void) => {
    readonly remove: () => void;
  };
}

export interface AmbientNotificationReceiptStore {
  readonly claim: (notificationId: string) => Promise<boolean>;
}

export type AmbientNotificationDisposition =
  | "opened"
  | "duplicate"
  | "foreign_workspace"
  | "invalid"
  | "ignored_action";

export const processAmbientNotificationResponse = async (input: {
  readonly response: AmbientNotificationResponse;
  readonly defaultActionIdentifier: string;
  readonly workspaceId: string;
  readonly store: AmbientNotificationReceiptStore;
  readonly openUrl: (url: string, payload: AmbientNotificationPayload) => Promise<void>;
}): Promise<AmbientNotificationDisposition> => {
  if (input.response.actionIdentifier !== input.defaultActionIdentifier) {
    return "ignored_action";
  }
  let payload: AmbientNotificationPayload;
  try {
    payload = decodeAmbientNotification(input.response.notification.request.content.data);
  } catch {
    return "invalid";
  }
  const claimed = await input.store.claim(payload.notificationId);
  if (!claimed) return "duplicate";
  if (payload.workspaceId !== input.workspaceId) return "foreign_workspace";
  await input.openUrl(notificationDeepLink(payload), payload);
  return "opened";
};

export const watchAmbientNotificationResponses = (input: {
  readonly source: AmbientNotificationSource;
  readonly workspaceId: string;
  readonly store: AmbientNotificationReceiptStore;
  readonly openUrl: (url: string, payload: AmbientNotificationPayload) => Promise<void>;
  readonly onDisposition?: (disposition: AmbientNotificationDisposition) => void;
}): (() => void) => {
  let active = true;
  let queue = Promise.resolve();
  const submit = (response: AmbientNotificationResponse): void => {
    queue = queue
      .then(async () => {
        if (!active) return;
        const disposition = await processAmbientNotificationResponse({
          response,
          defaultActionIdentifier: input.source.defaultActionIdentifier,
          workspaceId: input.workspaceId,
          store: input.store,
          openUrl: input.openUrl,
        });
        if (active) input.onDisposition?.(disposition);
      })
      .catch(() => undefined);
  };

  const subscription = input.source.addResponseListener(submit);
  void input.source
    .getLastResponse()
    .then((response) => {
      if (response !== null) submit(response);
    })
    .catch(() => undefined)
    .finally(() => input.source.clearLastResponse().catch(() => undefined));

  return () => {
    active = false;
    subscription.remove();
  };
};
