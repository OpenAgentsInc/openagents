# EP250-A selected-Pylon admission receipt

Issue: [#8718](https://github.com/OpenAgentsInc/openagents/issues/8718)

## Outcome

A successful Khala coding delegation now identifies the Pylon that the server
actually admitted. The SSE response carries
`openagents-selected-pylon-ref` from `delegation.pylon.pylonRef`, and
`pylon khala request --json` projects it as `selectedPylonRef`.

This is admission truth, not a reflection of caller input. It works when the
caller requests an explicit Pylon and when the server selects linked capacity.
Older gateways that omit the additive header remain compatible and project
`selectedPylonRef: null`. Pylon rejects malformed non-null header values before
including them in its public-safe result.

No dispatch, ownership, or capacity policy changed, so this slice does not
alter an invariant or the wire protocol version.

## Verification

- Worker chat-completions route: 184 tests passed.
- Pylon Khala requester: 41 tests passed.
- Worker TypeScript typecheck passed.
- Pylon TypeScript typecheck passed.
- `git diff --check` passed.

The tests cover explicit admission, resolver-selected admission, CLI JSON
projection, legacy header absence, and malformed-header rejection without
opening a product app.
