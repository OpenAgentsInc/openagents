# Decision protobuf projection v1alpha1

`systemone.proto` is a protobuf projection of the frozen JSON contract
`openagents.systemone.v1` (NIP-DEC, `POST /v1/systemone`), following the
`decision.v1` shapes in section 16.1 of the
[protocol spec](../../../../docs/protocol/openagents-protocol-spec.md). The JSON
contract stays the definition. It is alpha until `decision.v1` is published
with its common types (`T.Null`, `T.Outcome`, `T.ArtifactRef`); until then a
local `Null` stands in.

Scope: `SystemOneRequest` with the `EntryType` tree and questions, and
`SystemOneResponse` with answers and the service identity. `ServiceIdentity`
adds `identity` (the served model's `name@sha256:<digest>`), which Psionic's
server reports in every answer. `SystemOneResult`, status, cancel, receipts and
the other decision services are not projected yet.

Psionic's server (psionic/lib/serve-proto.coax) accepts a request with
`Content-Type: application/x-protobuf` by transcoding it to the NIP-DEC JSON
and admitting that, so the protobuf door never admits what the JSON door
refuses. It answers in protobuf when `Accept: application/x-protobuf`.
`EntryType` numbers must be integers within 2^53, the same set chapter 5's
text format accepts; probabilities carry the JSON answer's values (at most
nine decimals), parsed back exactly. The descriptor is
`proto/descriptors/systemone.pb`, profile `systemone` in `modules.json`.
