NIP-DS
======

Datasets
--------

`draft` `optional`

This NIP defines a protocol for publishing, selling, negotiating, and
delivering datasets on Nostr.

Nostr already has several pieces that can participate in data exchange:

- NIP-94 for file metadata
- NIP-15 and NIP-99 for marketplace and listing surfaces
- NIP-28 for public discussion
- NIP-17, NIP-44, and NIP-59 for private negotiation and private delivery
- NIP-90 for request/result and quote/delivery flows

What it does not yet have is a canonical dataset primitive.

This NIP introduces that primitive.

The goal is to make datasets, artifacts, and reusable context bundles
first-class objects on Nostr so that humans and agents can move data back and
forth with:

- stable identity
- verifiable digests
- explicit access posture
- discoverable offers
- interoperable negotiation
- safer delivery defaults

In practical terms, this NIP is not only about "files for sale." It is about
making useful context legible and tradable. That includes public datasets, but
also private research bundles, stored conversations, local project context,
evaluation corpora, and other knowledge packages that may need to flow between
people, teams, services, and agents.

The point is to establish best practices for how data should move on Nostr:

- how it is identified
- how it is described
- how it is previewed
- how it is offered
- how it is requested
- how it is delivered
- how it is discussed publicly or negotiated privately

One protocol should support more than one market shape:

- sometimes a seller wants a persistent public catalog item or "market stall"
- sometimes a seller wants targeted access under explicit policy
- sometimes a buyer wants an on-demand request/quote/delivery flow
- sometimes both parties need public or private negotiation before delivery

This NIP separates those concerns into a small core plus optional profiles:

- DS core defines canonical dataset identity, listing metadata, and access
  offers
- DS-Market profiles reuse NIP-15 and NIP-99 for storefront and listing UX
- DS-DVM reuses NIP-90 for targeted access requests and result delivery
- DS-Chat reuses NIP-28 for public negotiation and NIP-17/NIP-59 for private
  negotiation

This NIP is designed to fit alongside:

- NIP-01: events, tags, subscriptions
- NIP-15: marketplace stalls and products
- NIP-17: private direct messages
- NIP-28: public chat channels
- NIP-32: labels and trust signals
- NIP-44 / NIP-59: encrypted terms and delivery pointers
- NIP-57: Lightning zaps
- NIP-60 / NIP-61 / NIP-87: Cashu and mint discovery
- NIP-89: handler announcements
- NIP-90: on-demand request/result flow
- NIP-94: file metadata
- NIP-99: classified listings

## Rationale

NIP-90 is a good fit for "I want access to this dataset now, under these
constraints, and I may need quoting, payment, or targeted delivery."

It is not, by itself, a good fit for "I want to put this thing up for sale and
let marketplaces or buyers discover it as a durable listing."

Likewise, NIP-15 and NIP-99 are good fits for storefront or classified
discovery, but they do not define canonical dataset identity, digests, preview
manifests, or targeted delivery semantics.

This NIP introduces a dataset-native identity and offer layer that can be used
with either model:

- catalog-first discovery through a persistent listing
- request-first access through NIP-90
- public discussion through NIP-28
- private terms and delivery through NIP-17 / NIP-44 / NIP-59

## Terms

- `seller` / `publisher`: the pubkey publishing a dataset listing or offer
- `buyer`: the pubkey seeking access to a dataset
- `dataset`: a single file or a multi-file bundle sold as one unit
- `dataset listing`: the canonical public identity and metadata for a dataset
- `dataset offer`: the access terms under which a dataset may be acquired
- `preview`: a sample, redacted subset, or metadata-only representation
- `delivery`: the act of providing the dataset payload or a pointer to it

## Scope And Layering

### DS Core

DS core is normative and defines:

- canonical dataset identity
- `kind:30404` dataset listings
- `kind:30405` draft/inactive dataset listings
- `kind:30406` dataset offers
- `kind:30407` dataset access contracts
- baseline discovery
- linkage to previews, manifests, offers, access contracts, and discussion
  channels

### Optional Profiles

DS core intentionally does not require a single checkout or delivery rail.

Optional profiles are:

- DS-Market via NIP-15 and NIP-99
- DS-DVM via NIP-90 request/result events
- DS-Chat via NIP-28 public channels

### Out Of Scope

This NIP does not standardize:

- a single blob-storage backend
- a single settlement rail
- arbitration or dispute courts
- a single packaging tool or manifest serializer
- indexer implementation details

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 30404 | Addressable | Dataset Listing |
| 30405 | Addressable | Draft / Inactive Dataset Listing |
| 30406 | Addressable | Dataset Offer |
| 30407 | Addressable | Dataset Access Contract |
| 5960  | Regular     | Dataset Access Request (optional DS-DVM profile) |
| 6960  | Regular     | Dataset Access Result (optional DS-DVM profile) |

This NIP also reuses:

| Kind  | NIP | Role |
| ----- | --- | ---- |
| 7000  | 90  | Dataset access feedback in the DS-DVM profile |
| 1063  | 94  | File metadata for previews, manifests, or public payload refs |
| 1985  | 32  | Labels for license, sensitivity, trust, or quality |
| 31990 | 89  | Optional dataset handler or provider announcements |
| 40-44 | 28  | Public negotiation / discussion channels |
| 14-15 | 17  | Private negotiation and encrypted file messaging |
| 1059  | 59  | Gift-wrapped private terms and delivery pointers |

## 1. Dataset Identity

### 1.1 Canonical Dataset Address

The canonical address of a dataset listing is:

```text
30404:<seller_pubkey>:<d-tag>
```

This identifies the current public head of the dataset listing.

### 1.2 Dataset Scope ID

For underwriting, access policies, or pinned delivery logic, implementations
SHOULD derive a versioned scope identifier:

```text
dataset_scope_id = 30404:<seller_pubkey>:<d-tag>:<sha256_digest>
```

Where `<sha256_digest>` is the value of the listing's `x` tag.

### 1.3 Canonical Digest Rule

Every active dataset listing MUST include an `x` tag containing the SHA-256
hex digest of the sold unit.

Recommended rules:

- for a single file dataset, `x` SHOULD be the SHA-256 of the file bytes
- for a multi-file dataset, `x` SHOULD be the SHA-256 of a canonical manifest
  over the bundle members
- the canonical manifest SHOULD use deterministic ordering by relative path and
  record, at minimum, `path`, `size`, `mime`, and each member's SHA-256 digest

If a listing also references NIP-94 file metadata events, those file events do
not replace the listing-level `x` digest. The listing digest is the canonical
identity anchor for the sold dataset unit.

## 2. Dataset Listing (`kind:30404`)

An addressable event keyed by `d` that defines the canonical public identity of
the dataset.

The `.content` field SHOULD be a Markdown description of:

- what the dataset is
- how it was assembled or redacted
- who is selling it
- what the buyer should expect

### 2.1 Required Tags

A valid `kind:30404` dataset listing MUST include:

- `d`: stable dataset slug or identifier
- `title`: human-readable dataset title
- `x`: canonical dataset digest as defined above
- `published_at`: first publication timestamp as a unix timestamp string

### 2.2 Recommended Tags

- `summary`: short description
- `version`: dataset version, release name, or date stamp
- `dataset_kind`: one of `table`, `corpus`, `image_collection`,
  `audio_corpus`, `video_corpus`, `conversation_bundle`, `embedding_set`,
  `eval_bundle`, `mixed`, or another seller-defined value
- `m`: primary MIME type or manifest MIME type
- `size`: total bytes
- `records`: row, example, or item count if known
- `license`: license or policy shorthand
- `access`: one of `open`, `paid`, `quote`, `targeted`, `subscription`,
  `negotiated`
- `delivery`: repeated delivery modes; see below
- `t`: searchable topics or categories; publishers SHOULD include `dataset`
- `e`: references to file metadata or discussion channels using DS markers
- `a`: references to dataset offers or market wrappers using DS markers

### 2.3 Delivery Tag Values

`delivery` tags declare the seller's supported delivery modes. Suggested
values:

- `download`
- `nip90`
- `nip94`
- `blossom`
- `giftwrap`
- `dm`
- `torrent`
- `manual`

### 2.4 DS Markers For `e` And `a` Tags

This NIP uses ordinary NIP-01 `e` and `a` tags with the fourth element used as
an optional DS marker.

Recommended markers:

- `manifest`: points to a NIP-94 manifest or file metadata event
- `preview`: points to a preview or sample event
- `payload`: points to a public/open payload event
- `discussion`: points to a NIP-28 channel create or channel metadata event
- `offer`: points to a `kind:30406` dataset offer
- `market`: points to a NIP-15 or NIP-99 market wrapper

### 2.5 Example

```json
{
  "kind": 30404,
  "content": "A cleaned Q1 2026 corpus of Bitcoin policy transcripts with redacted personal identifiers and a metadata-only preview.",
  "tags": [
    ["d", "bitcoin-policy-transcripts-q1-2026"],
    ["title", "Bitcoin Policy Transcripts Q1 2026"],
    ["summary", "Redacted transcript corpus with speaker and source metadata."],
    ["published_at", "1774080000"],
    ["version", "2026-q1"],
    ["dataset_kind", "corpus"],
    ["m", "application/x-ndjson"],
    ["x", "<sha256-bundle-digest>"],
    ["size", "28444192"],
    ["records", "4127"],
    ["license", "seller-license-v1"],
    ["access", "paid"],
    ["delivery", "nip90"],
    ["delivery", "download"],
    ["t", "dataset"],
    ["t", "bitcoin"],
    ["t", "transcripts"],
    ["e", "<nip94-preview-event-id>", "<relay>", "preview"],
    ["e", "<kind41-discussion-event-id>", "<relay>", "discussion"],
    ["a", "30406:<seller-pubkey>:open-offer", "<relay>", "offer"]
  ],
  "pubkey": "<seller-pubkey>"
}
```

## 3. Draft / Inactive Listing (`kind:30405`)

`kind:30405` has the same structure as `kind:30404`, but it is used for:

- unpublished drafts
- inactive listings
- paused or hidden listings

Draft listings MAY omit `published_at` until the first active publication.

Clients SHOULD NOT treat `kind:30405` as an active public offer unless local
policy explicitly says otherwise.

## 4. Dataset Offer (`kind:30406`)

An addressable event keyed by `d` that defines the access terms for a dataset.

One dataset listing MAY have multiple active offers, for example:

- a public open offer
- a targeted offer for a known buyer
- a quote-only offer
- a subscription offer
- a NIP-90-only offer

The `.content` field SHOULD describe the human-facing terms:

- what access is being sold
- what the buyer receives
- what delivery mode applies
- any restrictions or obligations

### 4.1 Required Tags

A valid `kind:30406` dataset offer MUST include:

- `d`: stable offer identifier
- `a`: the referenced dataset listing address
- `status`: one of `active`, `inactive`, `revoked`, `expired`
- at least one `delivery` tag

### 4.2 Recommended Tags

- `policy`: one of `open_offer`, `targeted_request`, `licensed_bundle`,
  `subscription`, `manual_review`, or another seller-defined value
- `price`: NIP-99 `price` tag shape
- `payment`: repeated accepted payment rails
- `p`: one or more targeted buyers if the offer is not public
- `expiration`: either NIP-40 or seller-defined time metadata
- `license`: license or grant identifier
- `t`: searchable categories

### 4.3 Payment Tag

Suggested `payment` tags:

- `["payment", "zap"]`
- `["payment", "ln"]`
- `["payment", "cashu", "<mint-url>"]`
- `["payment", "fedimint", "<federation-id>@<domain>"]`

If no `payment` tag is present, buyers MUST assume payment negotiation is
out-of-band or seller-defined.

### 4.4 Open And Targeted Offers

- If an offer has no `p` tags, it is an open offer.
- If an offer includes one or more `p` tags, it is targeted to those buyers.
- If an offer has no `price` tag, it is quote-only or free, depending on seller
  policy and `.content`.

### 4.5 Example

```json
{
  "kind": 30406,
  "content": "Targeted access for the full corpus. Delivery occurs after payment through a NIP-90 access result containing an encrypted delivery pointer.",
  "tags": [
    ["d", "targeted-offer-buyer-1"],
    ["a", "30404:<seller-pubkey>:bitcoin-policy-transcripts-q1-2026", "<relay>"],
    ["status", "active"],
    ["policy", "targeted_request"],
    ["price", "5000", "SAT"],
    ["payment", "zap"],
    ["payment", "cashu", "https://mint.example"],
    ["delivery", "nip90"],
    ["delivery", "giftwrap"],
    ["license", "seller-license-v1"],
    ["p", "<buyer-pubkey>", "<relay>"]
  ],
  "pubkey": "<seller-pubkey>"
}
```

## 5. Discovery

### 5.1 Baseline Discovery

Clients discover datasets by querying `kind:30404` directly.

Recommended query shape:

```json
["REQ", "datasets", {"kinds": [30404], "#t": ["dataset"], "limit": 200}]
```

For a specific dataset:

```json
["REQ", "dataset-by-d", {"kinds": [30404], "#d": ["bitcoin-policy-transcripts-q1-2026"], "authors": ["<seller-pubkey>"]}]
```

### 5.2 Offers

Clients discover offers by querying `kind:30406`, usually after resolving a
dataset listing:

```json
["REQ", "dataset-offers", {"kinds": [30406], "#a": ["30404:<seller-pubkey>:bitcoin-policy-transcripts-q1-2026"]}]
```

### 5.3 NIP-94 Linkage

Listings and offers MAY reference `kind:1063` NIP-94 file metadata events for:

- previews
- bundle manifests
- openly downloadable payloads
- alternative transport sources

When doing so, clients SHOULD still verify the DS listing or delivery digest
before treating the payload as authentic.

## 5.4 Dataset Access Contract (`kind:30407`)

An addressable event keyed by `d` that represents the durable lifecycle of a
specific buyer's access to a dataset.

This event exists to make post-sale state relay-native. It is the DS object
that bridges:

- the public listing identity
- the chosen offer
- the concrete buyer request
- payment-required and paid state
- delivered pointer metadata
- revocation, expiry, or refund outcomes

The seller SHOULD publish the current access-contract head for a
buyer-and-sale-specific `d` tag. Clients SHOULD treat it as the canonical relay
object for lifecycle state after a DS listing or DS offer has moved beyond
mere discovery.

### 5.4.1 Required Tags

A valid `kind:30407` dataset access contract MUST include:

- `d`: stable contract identifier
- `a`: the referenced dataset listing address, marked `listing`
- `e`: the referenced DS-DVM request event, marked `request`
- `status`: one of `payment-required`, `paid`, `delivered`, `revoked`,
  `expired`, `refunded`
- `p`: the buyer pubkey

### 5.4.2 Recommended Tags

- `a`: optional dataset offer address, marked `offer`
- `e`: optional DS-DVM result event, marked `result`
- `e`: repeated settlement evidence refs, marked `payment`
- `payment`: selected rail for this contract
- `amount`: settled or requested amount in millisats, with optional BOLT11
  invoice in the third field
- `delivery`: selected delivery mode
- `delivery_ref`: delivery pointer, receipt URL, gift-wrapped event id, or
  other seller-defined locator
- `m`: MIME type of the delivered payload or manifest
- `x`: delivered payload or manifest digest
- `expires_at`: unix timestamp after which the contract should be treated as
  expired
- `reason_code`: seller-defined revocation, refund, or expiry reason
- `a`: additional DS-linked refs for replacement chains, discussion, or local
  policy metadata

### 5.4.3 Marker Rules

This event uses canonical markers so clients can project lifecycle state
without ambiguous tag guessing.

Recommended markers are:

- `listing` on the canonical dataset listing `a` tag
- `offer` on the chosen dataset offer `a` tag
- `request` on the DS-DVM request `e` tag
- `result` on the DS-DVM result `e` tag
- `payment` on settlement-evidence `e` tags

### 5.4.4 Status Meaning

- `payment-required`: the seller matched the request and has published payment
  terms or an invoice, but settlement is not yet observed
- `paid`: the seller observed settlement and access is now active, even if
  delivery has not yet completed
- `delivered`: the seller has published or transmitted delivery metadata and
  the buyer can fetch or consume the payload
- `revoked`: previously granted access has been revoked
- `expired`: the access window expired without continued validity
- `refunded`: the seller or market considers the contract settled in the
  buyer's favor without continuing access

### 5.4.5 Example

```json
{
  "kind": 30407,
  "content": "Paid access contract for the full corpus. Delivery is available through an encrypted pointer after Lightning settlement.",
  "tags": [
    ["d", "buyer-1-q1-2026-access"],
    ["a", "30404:<seller-pubkey>:bitcoin-policy-transcripts-q1-2026", "<relay>", "listing"],
    ["a", "30406:<seller-pubkey>:targeted-offer-buyer-1", "<relay>", "offer"],
    ["e", "<5960-request-event-id>", "<relay>", "request"],
    ["e", "<6960-result-event-id>", "<relay>", "result"],
    ["e", "<payment-evidence-event-id>", "<relay>", "payment"],
    ["status", "delivered"],
    ["p", "<buyer-pubkey>", "<relay>"],
    ["payment", "ln"],
    ["amount", "5000", "lnbc50n1..."],
    ["delivery", "encrypted_pointer"],
    ["delivery_ref", "https://delivery.example/contracts/abc123"],
    ["m", "application/x-ndjson"],
    ["x", "<sha256-bundle-digest>"],
    ["expires_at", "1774166400"]
  ],
  "pubkey": "<seller-pubkey>"
}
```

Clients that support DS-DVM SHOULD use the access contract as the durable
post-sale state object and treat `5960`, `7000`, and `6960` as the request,
feedback, and fulfillment transport around it.

## 6. DS-Market Profiles

DS core intentionally does not replace NIP-15 or NIP-99.

It gives them a canonical dataset object to point at.

### 6.1 NIP-15 Market Stalls And Products

Sellers MAY expose datasets through NIP-15 storefronts.

Recommended mapping:

- `kind:30017` stall = seller storefront
- `kind:30018` product = market SKU for a dataset listing or offer

A NIP-15 product SHOULD include an `a` tag referencing either:

- the dataset listing: `30404:<seller_pubkey>:<d-tag>`
- the dataset offer: `30406:<seller_pubkey>:<d-tag>`

NIP-15 remains responsible for storefront UX and merchant/customer order flow.
DS remains the canonical source of dataset identity, digest, preview linkage,
and delivery posture.

### 6.2 NIP-99 Classified Listings

Sellers MAY advertise datasets through `kind:30402` classified listings.

A NIP-99 listing SHOULD include an `a` tag referencing the DS dataset listing
or offer. Clients SHOULD treat the DS object as canonical for digest, preview,
and delivery semantics.

Recommended NIP-99 tags for dataset ads:

- `["t", "dataset"]`
- `["a", "30404:<seller_pubkey>:<d-tag>"]`
- `["a", "30406:<seller_pubkey>:<offer-d-tag>"]` when applicable

### 6.3 Profile Selection Guidance

- use DS core when you need canonical dataset identity and offers
- use NIP-15 when you want a storefront or cart-like merchant UX
- use NIP-99 when you want a simple classified ad
- use DS-DVM when you want targeted, request/quote/delivery behavior

## 7. Optional DS-DVM Profile (NIP-90)

This NIP defines an optional dataset-access request/result profile on top of
NIP-90.

Use this profile when the buyer needs:

- targeted access
- quote-first access
- policy evaluation before delivery
- encrypted delivery pointers
- request-scoped negotiation

### 7.1 Dataset Access Request (`kind:5960`)

A buyer requests dataset access by publishing a `kind:5960` event.

The request SHOULD include:

- `a` tag referencing the dataset listing and/or dataset offer
- optional `p` tag targeting a seller
- optional `output` tag, typically `application/json`
- optional `bid` tag as defined in NIP-90
- optional `relays` tag for response publication
- optional `param` tags for delivery mode, preview posture, or license ack

Example:

```json
{
  "kind": 5960,
  "content": "",
  "tags": [
    ["a", "30404:<seller-pubkey>:bitcoin-policy-transcripts-q1-2026", "<relay>"],
    ["a", "30406:<seller-pubkey>:targeted-offer-buyer-1", "<relay>"],
    ["p", "<seller-pubkey>", "<relay>"],
    ["output", "application/json"],
    ["param", "delivery", "giftwrap"],
    ["param", "preview", "metadata_only"],
    ["param", "license_ack", "seller-license-v1"],
    ["bid", "5000000"],
    ["relays", "wss://relay.example.com"]
  ],
  "pubkey": "<buyer-pubkey>"
}
```

If request details are sensitive, clients MAY encrypt NIP-90 `i` and `param`
payloads as described in NIP-90.

### 7.2 Dataset Access Result (`kind:6960`)

A seller responds with a `kind:6960` result event.

The result SHOULD include:

- `request` tag carrying the original request event as stringified JSON
- `e` tag referencing the request
- `p` tag referencing the buyer
- `a` tag referencing the dataset and/or offer
- optional `amount` tag if payment is requested or confirmed
- optional `x` tag for the delivered bundle digest

The `.content` SHOULD be one of:

- a JSON delivery descriptor
- an encrypted delivery pointer
- an encrypted preview or metadata sample

Suggested JSON delivery descriptor:

```json
{
  "dataset": "30404:<seller-pubkey>:bitcoin-policy-transcripts-q1-2026",
  "offer": "30406:<seller-pubkey>:targeted-offer-buyer-1",
  "delivery": "download",
  "ref": "https://download.example.com/receipt/abc123",
  "mime": "application/x-ndjson",
  "x": "<sha256-bundle-digest>",
  "expires_at": 1774166400,
  "license": "seller-license-v1"
}
```

### 7.3 Feedback (`kind:7000`)

The DS-DVM profile reuses NIP-90 `kind:7000` feedback.

Recommended status usage:

- `payment-required`
- `processing`
- `success`
- `partial`
- `error`

The optional third value in the `status` tag MAY carry DS-specific detail such
as:

- `dataset-not-available`
- `offer-revoked`
- `quote-required`
- `preview-available`
- `scope-mismatch`

### 7.4 Handler Announcements (NIP-89)

Sellers or dataset gateways MAY advertise DS-DVM support using `kind:31990`
with `k=5960`.

Recommended tags:

- `["k", "5960"]`
- `["t", "dataset"]`
- `["a", "30404:<seller_pubkey>:<d-tag>"]` for one or more supported datasets

## 8. DS-Chat Profile

### 8.1 Public Negotiation With NIP-28

Sellers MAY create a NIP-28 public chat channel for a dataset listing, offer,
or seller storefront.

Recommended pattern:

- create a `kind:40` channel
- publish `kind:41` metadata including:
  - `["a", "30404:<seller_pubkey>:<d-tag>", "<relay>"]`
  - optional `["a", "30406:<seller_pubkey>:<offer-d-tag>", "<relay>"]`
  - `["t", "dataset"]`
  - `["t", "nip-ds"]`

The dataset listing SHOULD link the channel using an `e` tag with the
`discussion` marker.

This makes it possible for buyers to:

- pull dataset-related channels from relays
- browse public Q&A and previews
- negotiate in public when appropriate

Example NIP-28 metadata event:

```json
{
  "kind": 41,
  "content": "{\"name\":\"Bitcoin Policy Corpus Q1 2026\",\"about\":\"Public Q&A for the dataset offer.\",\"relays\":[\"wss://relay.example.com\"]}",
  "tags": [
    ["e", "<kind40-channel-create-id>", "<relay>", "root"],
    ["a", "30404:<seller-pubkey>:bitcoin-policy-transcripts-q1-2026", "<relay>"],
    ["t", "dataset"],
    ["t", "nip-ds"]
  ],
  "pubkey": "<seller-pubkey>"
}
```

### 8.2 Private Negotiation And Delivery

Public channels are useful for:

- discovery
- trust building
- clarifying terms
- public support

They are not appropriate for:

- private dataset contents
- buyer credentials
- payment secrets
- private delivery refs

For those, clients SHOULD use:

- NIP-17 private direct messages
- NIP-59 gift wrap
- NIP-44 encrypted payloads

Private messages SHOULD reference the dataset or offer using `a` tags so the
conversation remains anchored to the public dataset identity.

## 9. Labels And Trust (NIP-32)

Publishers and third parties MAY label dataset listings or offers using
`kind:1985` labels.

Suggested namespaces:

- `dataset/license`
- `dataset/sensitivity`
- `dataset/quality`
- `dataset/trust`

Suggested labels:

- `public-domain`
- `cc-by`
- `licensed`
- `redacted`
- `synthetic`
- `pii`
- `medical`
- `hash-verified`
- `preview-only`
- `malware-risk`

Labels MAY target:

- the dataset listing via `a`
- the dataset offer via `a`
- a preview or payload file via `e`

## Security Considerations

### Canonical Identity

Market wrappers and chat threads are not canonical identity. Clients SHOULD
treat the DS dataset listing and its `x` digest as canonical.

### Digest Verification

Clients MUST verify delivered payloads against the expected digest before
treating them as authentic.

### Metadata Leakage

Dataset listings, public offers, and NIP-28 channels are public. Sellers SHOULD
not place private buyer identity, confidential terms, or private payload
details there.

### NIP-90 Access Leakage

The DS-DVM profile can reveal access patterns and buyer interest. Sensitive
requests SHOULD encrypt request parameters and use private relays where
possible.

### Off-Band URLs

HTTP, Blossom, torrent, or other transport pointers can change or disappear.
The digest anchor and optional NIP-94 metadata references help clients verify
what was actually delivered.

### Private Delivery

When delivery refs or terms are private, implementations SHOULD use NIP-44 or
NIP-59 and relays that protect recipient metadata.

## References

- NIP-01: Basic protocol
- NIP-15: Marketplace stalls and products
- NIP-17: Private direct messages
- NIP-28: Public chat
- NIP-32: Labeling
- NIP-44: Encrypted payloads
- NIP-57: Lightning zaps
- NIP-59: Gift wrap
- NIP-60 / NIP-61 / NIP-87: Cashu ecosystem
- NIP-89: Recommended application handlers
- NIP-90: Data vending machines
- NIP-94: File metadata
- NIP-99: Classified listings

## Changelog

**v1**

- Initial draft.
