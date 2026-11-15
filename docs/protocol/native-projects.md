# Native project management

The native client uses the web's existing project list, private-project page
operations and project drops. Existing published contracts keep their bytes.
`coax/lib/project/wire.coax` reads private page cells; the browser's private
reader calls the same cell codec; both private pages index their lines with
`pj_index_fit`. The JSON parser/writer and field walk (`components/adapters/json-fields.coax`)
and the draft review rows (`coax/lib/project/draft.coax`) are shared: the web
page and the native app draw the same section headings, group headers, readable
names, "+ add" rows and save bodies (`{"path":[...],"value":"..."}`). The browser
only sends the draft JSON (`V|state|edits|note|name|grow|bytes` then the JSON) and
posts the body the page builds. Both read drafts up to 256 KB and 8192 values and
show up to 4096 fields; past a bound the review says so on screen with the count.
A failed or oversized response is an error, never an empty project.

Public creation is the website's drop → read → review → explicit publish
sequence. Add to project creates a grow drop with its public project key.
Pending drops resume from the server. Notes are a UTF-8 Markdown attachment;
other files come from the system picker. The server's existing may/size/scan,
draft validation and membership rules remain authoritative. A declined file
or failed upload never starts drafting or publication. Every draft field is
editable before publication; Publish and Discard have explicit confirmation.
A lost mutation reply is reconciled with a read, without automatically sending
that mutation again. A draft is never implicitly published on navigation.

Member-only projects use their existing state/talk lines and ask, stop, undo,
agree, status and message operations. The shared project-agent conversation
stays in the app, with Markdown and collapsed tool/event components. Agent
availability, pending work, undoability and agreement are server observations.
Private contents are cleared on account changes and refused access. No public
fallback follows a private authorization failure except a 404 public project
lookup; that lookup requests only the generic published Markdown path.

Public projects additionally have account-owned saved project chats. A separate
`openagents.chat.project-link.v1` associates an existing saved v1 thread with a
project. A thread cannot be reassigned to another project. The unchanged shared
history format lists those threads. `openagents.chat.project-turn.v1` wraps the
unchanged thread and slug for inference with server-fetched project context;
funding and ownership remain the existing account-chat rules. Saving a user
turn, linking it and inference are serialized; a failed link never starts the
model. Project conversations are reopened in the existing native transcript.

`coax.native.documents.v1` is an installed capability, not an OTA filesystem
API: user-chosen bytes have one ephemeral opaque lease. PUT carries no app
authorization/cookies and is limited to the storage endpoint; the existing
no-redirect upload session is reused. A new native runtime/build is required.
UI/state and future compatible screen updates remain signed OTA resources.

## Email-only project owners

`openagents.project.account-owner.v1` gives a verified person account one stable
opaque 64-hex owner handle, retained in `drops.account_owners_v1`. It is not a
Nostr signing key or authenticator. The handle hashes the domain
`openagents.project.account-owner.v1:` and the opaque account ID; it never
contains an email. Linking an authenticator later does not move ownership.
Existing Nostr member documents and project signing keys retain their identities.
The API resolves public ownership through this account mapping and private
membership through the existing canonical linked Nostr key. New email project
drops use the existing creator grant; no admin, key-login or agent rights are
created. Existing owners retain reads when creation is revoked. Account export,
merge and deletion include the mapping.

The internal lifecycle registry’s legacy `pubkey` resolver includes these
account ownership handles when finding key-addressed drop records. Export and
purge therefore reach their drops, files and drafts; `auth.strong_ways` and
Nostr authentication still read only actual authenticators.
