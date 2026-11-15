# Arcade account/settings port — 1088

The source reference remains Arcade `3f10e1e`, with `app/arclib` at
`542ecec`. This implements the owner’s account/settings and drawer request;
the larger Nostr inbox, private messaging and channel/group work in the port
plan remains separate.

| Arcade source | OpenAgents implementation |
| --- | --- |
| ProfileScreen | Account hub: Your profile, Keys, Devices; Settings; Resources; confirmed logout |
| EditProfileScreen | Native display name, username, NIP-05, about, picture/banner HTTPS URLs and website. Revisioned account save and explicit kind-0 publication to saved relays |
| RelayManagerScreen | Saved WSS relays, custom addition, duplicate/URL checks, confirmed removal, at least one and at most six. Labels say saved, not connected |
| PrivacySettingScreen | Actual remote-image and private-clipboard preferences. The obsolete proprietary private-DM mode is excluded |
| NotificationSettingScreen | iOS permission, security-notice push registration, system settings and disabling this device’s push. No unsupported DM/channel notification switches |
| BackupScreen | Account identity or individually held login-key backup, masked hex/nsec, explicit reveal and separate expiring local copies; clears on dismissal, background/capture and expiry |
| LoginScreen / CreateAccount | Existing email-code account creation with its managed identity; native nsec/hex login. Existing accounts can link/create additional login keys and add/remove passkeys |
| ChangelogScreen / resources | OpenAgents changelog, About, Updates, Components, Extras, GitHub and website |
| ScreenWithSidebar / DrawerIconButton | 70-point rail, 50-point circular icons, main-surface translation, finger-tracked edge/closing drags, velocity/halfway settling, backdrop/escape dismissal, RTL, reduced motion and shared GPU hamburger/arrow pose |

Chat, Projects and Worlds retain their existing destinations. Recent chats
opens the readable history pane, and wider windows retain a history column.
Settings closes the drawer before opening, and returning preserves chat drafts.
The shared monochrome skin and native component scene paint the new controls.

The shared UI is `apps/shared/account-settings.coax`; the iOS app only routes
it. Native key/SDK effects are `NativeAccount.swift`, with Coax NIP-19, NIP-98,
NIP-01 event and relay-ack codecs in `coax/native/account-secrets.coax`.
Secret fields never send editing events or values to the updateable UI.
The saved account codec and authority are `account-settings-wire.coax` and
`account-settings-policy.coax`. These belong to Identity in both context maps.
No parser, crypto primitive, edit buffer or component is copied from Arcade.

Account-managed identities remain distinct from authenticators and wallet
seeds. Importing another login key does not rotate the canonical identity.
Managed backup requires fresh L2 plus device authentication; a failed proof
cannot fall back to local export. Independent local login-key backup uses
explicit device authentication. Passkey/key removal retains the existing
last-method and fresh-verification rules.

Photos are edited using HTTPS image URLs; Arcade’s legacy `nostrimg.com`
uploader is not adopted. The native image adapter uses credential-free,
redirect-free bounded downloads and ImageIO thumbnails. Private preferences
never enter public kind-0 metadata. Server routes never fetch arbitrary relays.
These are OpenAgents account profiles; existing external relay metadata is not
automatically imported.

Protocol: [account settings v1](../protocol/account-settings-v1.md).
Release evidence: [1088](../verification/2026-11-14-arcade-settings/README.md).
