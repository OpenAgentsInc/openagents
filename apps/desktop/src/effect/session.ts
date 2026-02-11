import { Context, Effect, Layer, Ref } from "effect";

export type DesktopSession = Readonly<{
  readonly userId: string | null;
  readonly token: string | null;
}>;

export type DesktopSessionApi = Readonly<{
  readonly get: () => Effect.Effect<DesktopSession>;
  readonly set: (next: DesktopSession) => Effect.Effect<void>;
  readonly clear: () => Effect.Effect<void>;
}>;

export class DesktopSessionService extends Context.Tag("@openagents/desktop/DesktopSessionService")<
  DesktopSessionService,
  DesktopSessionApi
>() {}

const emptySession = (): DesktopSession => ({
  userId: null,
  token: null,
});

export const DesktopSessionLive = Layer.effect(
  DesktopSessionService,
  Effect.gen(function* () {
    const ref = yield* Ref.make<DesktopSession>(emptySession());
    return DesktopSessionService.of({
      get: () => Ref.get(ref),
      set: (next) => Ref.set(ref, next),
      clear: () => Ref.set(ref, emptySession()),
    });
  }),
);
