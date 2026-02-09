import { Atom, Result } from '@effect-atom/atom';
import { Cause, Effect } from 'effect';
import { ChatService } from '../chat';
import { AppAtomRuntime } from './appRuntime';

import type { ChatSnapshot } from '../chat';

/** Current user's owned thread id (set when logged in and after ensureOwnedThread). Null when not loaded or not logged in. */
export const OwnedThreadIdAtom = Atom.make<string | null>(null).pipe(
  Atom.keepAlive,
  Atom.withLabel('OwnedThreadIdAtom'),
);

const EMPTY_SNAPSHOT: ChatSnapshot = {
  messages: [],
  status: 'ready',
  errorText: null,
};

export const ChatSnapshotResultAtom = Atom.family((chatId: string) =>
  AppAtomRuntime.subscriptionRef(
    Effect.gen(function* () {
      const chat = yield* ChatService;
      return yield* chat.open(chatId);
    }),
  ).pipe(Atom.keepAlive, Atom.withLabel(`ChatSnapshotResultAtom(${chatId})`)),
);

export const ChatSnapshotAtom = Atom.family((chatId: string) =>
  Atom.make((get) => {
    const result = get(ChatSnapshotResultAtom(chatId));
    if (Result.isSuccess(result)) return result.value;
    if (Result.isFailure(result)) {
      const pretty = Cause.pretty(result.cause as Cause.Cause<unknown>).trim();
      return { ...EMPTY_SNAPSHOT, status: 'error', errorText: pretty || 'Chat failed.' } satisfies ChatSnapshot;
    }
    return EMPTY_SNAPSHOT;
  }).pipe(Atom.keepAlive, Atom.withLabel(`ChatSnapshotAtom(${chatId})`)),
);

export const AutopilotChatInputAtom = Atom.family((chatId: string) =>
  Atom.make('').pipe(Atom.keepAlive, Atom.withLabel(`AutopilotChatInputAtom(${chatId})`)),
);

export const AutopilotChatIsAtBottomAtom = Atom.family((chatId: string) =>
  Atom.make(true).pipe(Atom.keepAlive, Atom.withLabel(`AutopilotChatIsAtBottomAtom(${chatId})`)),
);

