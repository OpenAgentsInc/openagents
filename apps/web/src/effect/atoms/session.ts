import { Atom } from '@effect-atom/atom';
import { Schema } from 'effect';

export class SessionUser extends Schema.Class<SessionUser>('SessionUser')({
  id: Schema.String,
  email: Schema.NullOr(Schema.String),
  firstName: Schema.NullOr(Schema.String),
  lastName: Schema.NullOr(Schema.String),
}) {}

export class Session extends Schema.Class<Session>('Session')({
  userId: Schema.NullOr(Schema.String),
  user: Schema.NullOr(SessionUser),
}) {}

/**
 * Server populates this from WorkOS auth, then we dehydrate/hydrate it across SSR.
 */
export const SessionAtom = Atom.make<Session>({ userId: null, user: null }).pipe(
  Atom.serializable({
    key: '@openagents/web/session',
    schema: Session,
  }),
);
