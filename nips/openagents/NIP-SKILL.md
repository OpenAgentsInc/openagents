# NIP-SKILL: Published skills

`draft` · kind `30188`

## Scope

A skill is a `SKILL.md` document an agent can use. openagents.com reviews
submitted skills (`tenancy::skills`: static checks, a decision review, and
the admission gates) and publishes the ones it admits. This profile makes
that directory readable by any client: each published skill is one
addressable event signed by the directory that admitted it.

A listing is a directory's statement that it reviewed and published this
version. It grants nothing and runs nothing; a reader that installs the
skill fetches the document and checks its digest.

## Skill listing (kind 30188)

Signed by the directory's key. Readers choose which directories they trust.

| Part | Contract |
| --- | --- |
| `d` tag | The skill's name: 1 to 64 lowercase letters, digits, and `-`. |
| `version` tag | The published version, semver. |
| `x` tag | The document's digest, `sha256:` and 64 lowercase hex characters. |
| `r` tag | The HTTPS URL that serves that exact document. |
| `p` tag | Optional. The author's Nostr key, so a reader can link the author's profile and list a person's skills with `{"kinds":[30188],"#p":[key]}`. |
| `t` tags | The category, then the tags, lowercase. |
| `status` tag | Optional. `withdrawn` retires the listing; readers drop it. |
| `alt` tag | `OpenAgents skill` (NIP-31). |
| `content` | JSON: `{name, version, digest, url, license, category, tags}`, or empty when withdrawn. At most 16,384 bytes. |

A reader takes the newest event per `(pubkey, d)` with a valid signature
from a directory it trusts, and at the same second prefers a withdrawn one.
Reviews of a skill are NIP-32 labels (kind `1985`) whose `a` tag names the
listing's address, `30188:<directory key>:<name>`.

## Implementation

openagents.com (`crates/openagents-web`, `profiles::skills`) signs listings
with its directory key for the skills `GET /v1/skills` publishes and lists
them on `/explore`. `nostr::skill_listing` builds and reads them.
