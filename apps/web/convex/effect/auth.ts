import { Effect, Option } from 'effect';
import { tryPromise } from './tryPromise';

import type { Auth, UserIdentity } from 'convex/server';

export interface EffectAuth {
  readonly getUserIdentity: () => Effect.Effect<Option.Option<UserIdentity>>;
}

export class EffectAuthImpl implements EffectAuth {
  constructor(private auth: Auth) {}

  readonly getUserIdentity = (): Effect.Effect<Option.Option<UserIdentity>> =>
    tryPromise(() => this.auth.getUserIdentity()).pipe(
      Effect.map(Option.fromNullable),
      Effect.catchAll(() => Effect.succeed(Option.none())),
    );
}
