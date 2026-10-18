# Android projects and chat ownership

Create one Rust owner per shared account, before mounting its first real cloud
chat. Keep it outside Activity/view lifetimes:

```kotlin
val sessions = CoreAndroidSessions(account, cloudListener)
val cloud = sessions.defaultChat()
// RecoveryHost's openChat callback mounts that existing cloud handle.
```

The exported Rust signatures are:

```rust
CoreAndroidSessions::new(Arc<CoreAccount>, Arc<dyn CloudChatListener>) -> Arc<Self>
CoreAndroidSessions::default_chat(&self) -> Result<Arc<CoreCloudChat>, CoreError>
```

`defaultChat` returns the same object every time. It does not call `new_chat`,
create a computer/session, replace credentials, or restart onboarding. The existing
Rust cloud-chat/account watchers retain those responsibilities. Construct the
account with A3's `CoreAccount::new_with_secure_storage` and the Application's single
`AndroidEncryptedFileStore`; E1 uses that same account and data directory. Do not
construct another account or cloud-chat owner on resume.

After the account's existing `connect` method creates a `CoreClient`, pass it to
`attachClient`. Rust checks its account/session binding. `view`, `acknowledge`, and
`activate` expose Rust Native menus; Kotlin forwards only instance/revision/key.
The draft owns computer/project/branch/provider/model/effort choices. It reads
projects and branches from the selected computer and uses
`Client::live_session_catalog`, which refuses failed/empty discovery instead of
substituting a bundled or cached catalog. Each of its two RPCs has a 20-second
timeout. New sessions use read-only sandbox permissions. Selecting another computer
clears incompatible project, branch, and model state; stale catalog/ref/folder
results cannot replace newer choices. Menus show at most 16 choices per page.

Use `AndroidSessionsHost` with the shell's CoroutineScope if useful. It serializes
view preparation through native acknowledgment, fences queued navigation by view
generation, and cancels its screen tasks on `retire`/`close`. It borrows the Rust
owner; these methods do not destroy the account-owned object. Only the shared
account owner closes/destroys that object after retiring all mounted consumers.
Route typed actions to the existing cloud/transcript views, shared project form,
system browser, or explicit command copy. The host's failure callback must preserve
typed availability/refusal messages and must not create a replacement account.

`sharing(listener)` reuses `CoreSharing`'s Rust Native form, including its existing
connection/grant approval flow. `chatHomes(chat)` and `savedChatHomes(history)`
reuse `CoreChatHomes` choices/parts. Its `ChatHomeAction` navigates an existing part
or supplies `CoreContinuationDraft`; `continueOnSelected(draft)` uses the currently
validated Rust draft with that object's existing room/grant transfer path. It does
not copy transcripts into an unrelated chat or invent another HTTP route.

## Source limits and queued checks

This checkpoint is source-only. Regenerate the shared UniFFI Kotlin binding before
compiling the new owner/actions; B1 owns Activity/Application/Gradle integration.
Add `../sessions` to its main source roots. The existing B1 coroutine dependency
supports the optional host. No build, fixture execution, emulator, or live account
check has run for this slice.

The own-computer card uses the existing Rust offer/copy. `DeviceView` does not
identify cloud-managed computers, and `CloudHostView` has no mesh device ID. This
slice therefore offers the card when no execution computer is present and never
marks it done by guessing from a computer name. Matching the exact automatic
own-versus-cloud dismissal needs an authoritative classification/mapping from the
Rust device/account owner. A folder listing likewise gives repository metadata
for its child rows, not the root itself; **Use this folder** appears after choosing
a child whose metadata the host supplied. Neither gap is represented as a device
pass or silently filled with Kotlin rules.

Queue focused client catalog and mobile draft/owner fixtures, production-surface
mobile build plus binding generation, and B1's focused Kotlin compilation. Then
qualify project creation/sharing, account switching, catalog failure, cloud lifecycle
and entitlements, continuation/grant denial, and Activity recreation using offline
fixtures or a dedicated test account with `X-OpenAgents-Test: 1`. Real checks must
remove their chats, projects, grants, and cloud computers. They never use the owner's
account or real computers. Owner/device steps belong to NEEDS_OWNER/After-close;
the unrun code checks and classification gap remain issue acceptance work.
