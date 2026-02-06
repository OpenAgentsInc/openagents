import { Atom } from '@effect-atom/atom';
import { Schema } from 'effect';

export class Session extends Schema.Class<Session>('Session')({
  userId: Schema.NullOr(Schema.String),
}) {}

/**
 * Server populates this from WorkOS auth, then we dehydrate/hydrate it across SSR.
 */
export const SessionAtom = Atom.make<Session>({ userId: null }).pipe(
  Atom.serializable({
    key: '@openagents/web/session',
    schema: Session,
  }),
);

