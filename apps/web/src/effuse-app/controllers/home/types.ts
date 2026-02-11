import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry";

import type { Session } from "../../../effect/atoms/session";
import type { ChatClient } from "../../../effect/chat";
import type { AppRuntime } from "../../../effect/runtime";

export type SessionState = {
  readonly read: () => Session;
  readonly write: (session: Session) => void;
};

export type HomeChatDeps = {
  readonly runtime: AppRuntime;
  readonly atoms: AtomRegistry;
  readonly sessionState: SessionState;
  readonly navigate: (href: string) => void;
  readonly signOut: () => void | Promise<void>;
  readonly chat: ChatClient;
  readonly refreshConvexAuth?: () => void | Promise<void>;
};
