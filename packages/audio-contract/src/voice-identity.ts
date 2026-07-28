import { Schema as S } from "effect";

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Generation = S.Int.check(S.isGreaterThanOrEqualTo(1), S.isLessThanOrEqualTo(2_147_483_647));

export const VoiceIdentitySchema = S.Struct({
  ownerRef: Ref,
  deviceRef: Ref,
  threadRef: Ref,
  sessionRef: Ref,
  generation: Generation,
});
export type VoiceIdentity = typeof VoiceIdentitySchema.Type;
