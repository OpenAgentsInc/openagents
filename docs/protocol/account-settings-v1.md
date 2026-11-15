# Account settings and native key services

The Arcade settings port keeps the OpenAgents email account and its existing
managed identity. It adds profile, relay, privacy, notification, account-method
and backup screens in the shared component system. Wallet seeds and device
refresh keys are separate and are never exported by these screens.

`GET /api/auth/settings/v1` reads the session account's settings;
`POST /api/auth/settings/v1` replaces them at an expected revision. The JSON
profile in `proto/verse/account-settings-v1.json` is also the stored shape.
The initial document has revision 0 and OpenAgents' relay. A conflicting
revision returns 409; invalid input changes nothing. No caller-supplied account
or identity can change the owner. Frozen, limited and non-human callers cannot
write. Profile and relay fields are public; private preferences never enter a
Nostr kind-0 event. Export/merge/purge follow the account lifecycle registry.

`POST /api/auth/identity/backup/v1` requires a live human session at L2 within
five minutes. It opens the account's existing sealed identity, verifies its
derived public key and returns the registered private backup record, with
private/no-store headers. A linked identity has no server secret and returns
409. It never mints or rotates a key. The public identity-v1 contract remains
unchanged and never includes secrets. Backup is an explicit new contract;
ordinary account exports still omit sealed/private identity data.

`POST /api/auth/settings/profile-event/v1` signs only the saved public profile
as NIP-01 kind 0 using an existing managed identity. It cannot sign arbitrary
kinds, tags, caller content or identities. The phone sends that public event
to the saved relays and counts matching NIP-01 OK acknowledgements; a saved
profile is not described as published without acceptance. Server routes never
connect to arbitrary caller-supplied relay addresses.

Native account effects use `openagents.native.account.v1`, operation 18.
The observation contains public account/settings/method/session metadata.
Imported and exported secrets remain in native memory or the Keychain and
never enter Coax/Wasm, a component tree, telemetry, logs or OTA storage.
The platform adapter invokes shared NIP-19 and key validation and existing
NIP-98/passkey services. An imported key links through the existing one-time
proof and last-method rules. It does not replace the managed identity.

Backup/import requires an explicit action. Backup also authenticates locally;
secret fields start masked, clear when dismissed/backgrounded and are covered
while screen capture is active. Secret clipboard copies are local-only with a
short expiry. Account-managed backup, independently held login keys and the
non-exportable device refresh key have distinct labels. Cancelling a ceremony
or failing storage/proof leaves the existing key and session intact.

Arcade's obsolete private-DM switch is not reproduced as an encryption mode.
Privacy controls remote profile images and clipboard behavior. Notifications
shows actual iOS permission/registration state and manages the existing
security-notice push target; this port does not invent delivery of Nostr DMs
before the messaging transport exists. Changelog/resources refer to OpenAgents.

On phones the drawer is an Arcade-style 70-point sliding icon rail. Chat,
Projects and Worlds remain its places; Recent chats opens a full history pane.
The main surface slides with the rail, gestures track progress and settle,
backdrop taps dismiss, and the shared three-line icon morphs to an arrow.
Wide layouts retain a history column. Safe areas, RTL, reduced motion, keyboard
dismissal and accessibility labels apply to both presentations.
