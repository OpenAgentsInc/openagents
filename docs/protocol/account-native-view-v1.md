# Native account view state v1

`openagents.account.native-view.v1` keeps public view preferences on a native
device. The desktop carrier stores it at
`~/.openagents/desktop/native-view-v1.json`, with owner-only permissions and
atomic replacement. Identity owns these device preferences; this record
carries no login or execution authority.

The [profile](../../proto/coax-native/account-native-view-v1.json) freezes the
closed JSON object. `coax/native/account-view-wire.coax` is its only codec.
The three values are the typed login email, the existing core's opaque rejoin
reference and the environment picker's existing saved choices. Their existing
readers still validate and apply them. Keeping an ID does not enroll a
computer, authorize a turn or authenticate an account.

Private custody is separate: login credentials, pairing grants and pending
identity changes retain their own contracts. This file has no member for
those values. A malformed, truncated, over-capacity or newer-version record
is ignored as a whole. The native carrier never translates or rewrites the
old desktop settings or credentials to recover from a bad view record.

The host reads before launch, and writes only when the encoded record changes.
A close, crash or force quit must not be the first opportunity to save a new
conversation reference. The iOS preferences adapter may continue using its
existing storage; this profile does not silently migrate that representation.
