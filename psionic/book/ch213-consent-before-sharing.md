# 213. Consent before sharing

> **Status: unimplemented — design draft.** Nothing here runs yet.

Session content is private by default. This chapter makes sure that no session
content leaves the machine unless the person opts in, per session and
revocably. DeepSeek Harness sends session-log suffixes to its API by default.
Psionic's harness never does.

## What this chapter covers

- Opt-in per session. The default is no sharing.
- Revocation. The person can withdraw consent, and later requests stop
  sharing at once.
- Explicit attribution metadata. The person can inspect exactly what metadata
  accompanies a request.
- An outbound check. Every outbound request is compared with the session's
  consent state before it is sent.

## Dependencies

- [Chapter 201](ch201-model-visible-means-logged.md): the log that records consent changes.
- [Chapter 202](ch202-keep-the-prefix-stable.md): metadata kept outside the model input.
- [Chapter 210](ch210-speak-the-protocols.md): protocols that might carry content to other services.

## Done means

- With no opt-in, a captured outbound sink receives no session content across
  a scripted session.
- After revocation, no later request carries session content.
- The metadata view lists every field sent with a request, and the captured
  requests contain no other fields.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
