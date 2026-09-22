# Operating policies

The public operational contract for a deployment: status, contacts,
versioning, capacity, and commercial terms. Where no hosted service
exists yet, this page says so rather than implying one.

## Status and incidents

No hosted public origin is operated today — `docs/agents/openapi.yaml`
states that no public endpoint is published. When one is, this section
names its status page and incident channel; until then, incidents are
the repository's public issue tracker and there is no separate status
surface to check.

## Contacts

- **Support**: the repository's public issue tracker is the support
  channel for the open-source deployment path.
- **Security**: report vulnerabilities through the repository's private
  security-advisory channel on GitHub, not the public tracker. Never
  attach credentials, payloads, or registry contents to any report.
- **Abuse**: same channel as security.

## Versions and changes

- `docs/agents/api-catalog.json` is the versioned contract — its
  `version` field is what `x-api-version` emits on every response, and
  its `unimplemented` list names advertised surfaces that do not exist.
- Envelope schemas are versioned in-band (`v` fields:
  `openagents.systemone.v1`, `openagents.classify.v1`,
  `openagents.feedback.v1`, `openagents.updates-subscription.v1`).
- Compatibility and deprecation — including the migration windows a
  breaking change must announce — are in
  [compatibility.md](compatibility.md).
- There is no separate changelog file; the git history and the
  catalog's version are the record. A hosted deployment that needs a
  human-readable changelog adds one when it opens.

## Capacity and rate limits

Capacity is configuration, not a sales tier — `gateway.md` enumerates
the knobs. The defaults a fresh install ships:

- `max_in_flight`: 64 forwards across every door.
- `max_body_bytes`: 1 MiB request bodies; 4 MiB backend responses.
- `max_questions`: 256 per call; `max_options`: 4096.
- `max_classify_inputs`: 1024 per call, with a matching per-tenant
  share; `max_tenant_classify_in_flight` unset.
- `forward_timeout_ms`: two minutes; reservations orphan after
  `reservation_ttl_secs` (five minutes).
- Per-door `capacity.requests_per_minute` and tenant
  `quota.requests_per_day` come from the registry's bindings — a `429`
  answers with `retry-after` seconds.

Refusals are typed (`refusal_codes` in the catalog); `Retry-After`
appears on `429` responses; a `429` from a door's rate window is the
capacity policy working, not an error in the caller.

## Commercial terms

No paid tier is offered yet — the catalog says so. `billing-terms.md`
is the terms document a paid launch must publish first: plans,
subscriptions, and the monetary ledger already exist behind the
`billing` config, and turning them on for a public origin before terms
are published is out of policy.

## Product updates

`PUT /v1/updates` is the opt-in subscription: verified (it binds to the
authenticated credential), with `product` and `support` consent kept
separate, and `DELETE` unsubscribe honored durably. No mail is sent by
this service — whoever operates the sender reads only
`status: subscribed` records with `topics.product: true` for marketing,
and `topics.support` never licenses marketing contact.
