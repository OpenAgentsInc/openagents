# Native account chat and composer

Build 1071 extends the UIKit/JavaScriptCore profile with the shared kit's
composer and account-owned text chat. It requires a new native runtime:
1070 has no multiline editor projection. App state, draft handling, chat
requests, transcript presentation and retries remain compiled Coax delivered
through signed Updates. The installed host adds no WebView.

The reference is T3 Code at `f313d0a6cb`, in
`apps/mobile/src/features/threads/ThreadComposer.tsx`,
`apps/mobile/src/components/ComposerToolbar.tsx` and
`apps/mobile/modules/t3-composer-editor/ios/T3ComposerEditorView.swift`.
Its MIT notice accompanies the shared native projection. DM Sans is shared
under `components/fonts/`, with its OFL and pinned package provenance.
No upstream scripts ran on the Mac. No upstream Markdown parser or edit
buffer was copied.

The regular phone profile follows these reference dimensions in points:

| Part | Geometry |
| --- | --- |
| Outer margin / keyboard gap | 12 horizontal / 6 bottom |
| Surface | 27 radius, 2 vertical padding, regular native glass |
| Compact row / hit targets | 44 height, minimum editor 36 |
| Text | DM Sans Regular 16, line height 23, 6 vertical inset, zero fragment padding |
| Expanded editor | 14 horizontal inset, toolbar below, four-line / 104-point cap |
| Send | 44 hit target, 30 circle, 16 semibold arrow, primary #346bf1 |
| Model | 44 height, 16 provider symbol; icon and chevron compact, name expanded |

`components/layout/native-composer.coax` owns geometry. The shared
`k_composer_begin/end` tree is retained by `NativeProgram`; `NativeComposer`
projects it with UITextView. The editor survives frames without replacing
its selection, undo stack or marked text. Keyboard docking uses UIKit's
keyboard layout guide. Long input scrolls inside the editor; the screen
never zooms. Dynamic Type keeps the editor usable at larger sizes.

The plus menu offers new chat and clear draft. The model menu names the
currently configured Gemini 4 Hydrogen. This first profile is text-only:
attachment upload and custom speech recognition are not enabled. The mic
is disabled with an accessibility hint to use keyboard dictation. T3's
large-control/sidebar-picker and iPad variants are not claimed as matched.
Source measurements and native screenshots establish this regular profile;
there is no claim of a zero-difference comparison against a running T3 app.

## Account chat contract

`proto/coder/account-chat-v1.json` defines `openagents.chat.thread.v1`.
The one codec is `coax/web/api/account-chat-wire.coax`. The new versioned
routes are separate from the existing admin chat API:

* `POST /api/chat/thread/v1` saves a transcript for the authenticated person.
  An empty id creates an opaque id; revision is compare-and-swap on updates.
* `GET /api/chat/thread/v1?id=ID` reads only that person's account record.
* `POST /api/chat/complete/v1` translates the thread through the existing
  gateway codec and calls the existing metered Vertex adapter. The current
  beta requires a team account. It keeps existing daily request/spend caps;
  it does not expose admin routes, model credentials or wallet credit.

`chat.account_threads_v1` owns the account directly, not a made-up Nostr key.
It participates in account export, merge and purge. The unchanged history
shape combines these threads with older conversations owned by an account's
linked keys. Older key-owned conversations remain listed; their full viewer
is still on the web. Unlinked legacy identities are never merged implicitly.

A conversation holds at most 32 messages / 48,000 decoded UTF-8 bytes,
16,384 bytes per message and 65,536 encoded bytes. The draft allows 4,096
bytes. Sending reserves room for an answer and refuses overflow explicitly.
The user turn saves before inference; only an accepted save clears the
draft. An answer is saved afterward. A failed answer save offers Save reply;
a failed model call offers an explicit retry of the same user turn.
Stop cancels the native HTTP task and invalidates its callback generation;
it cannot promise that an already running upstream computation is unbilled.
No automatic model retries occur. A lost create acknowledgement can leave
an extra saved conversation, and a crash between model reply and saving
can lose that unsaved answer. Client transcripts are not trusted billing
receipts; inference's own records remain authoritative.

## Verification

Coax checks cover transitions, draft preservation, reply parsing and limits.
The account ownership/funding/revision authority requires zero surviving
mutants. The API harness exercises actual email authentication, PostgreSQL
ownership/CAS and the existing inference adapter under ASan/UBSan with a
fake upstream. XCTest exercises native geometry, editor retention and
email → compose → send → reply → relaunch → reopen. The portable artifact
is compared with real WebAssembly before release. Production model service
verification and owner phone acceptance are recorded with the release.


## Native messages, build 1072

The phone adapts T3 Code's `ThreadFeed.tsx` and
`native/EnrichedMarkdownText.tsx` at the same reference commit: selectable
native text, right-aligned user bubbles, unframed assistant prose, separate
code and table views, copy actions, stable message views and end-follow
scrolling that yields when the reader scrolls back. It reuses
`components/core/markdown.coax`, `components/kit/frames.coax` and the
transcript's extracted `components/core/scroll.coax`; no T3 parser or second
Markdown implementation is installed. Its MIT notice is retained with the
shared UIKit projection. Font provenance and OFL are in `components/fonts/`.

`k_message` builds semantic articles. The one parser retains heading levels,
combined inline marks, nested-list/task metadata and table alignment. It
can opt into thematic-break blocks; existing consumers retain their old
rule omission. Native TextKit lays out those semantics, preserves selection,
and supports horizontal scrolling for wide code and tables. Code-copy uses
raw code, while message-copy uses the original message source. A bounded
rich-tree fallback shows the complete source if the shared tree budget or
parser block/run budget would be exceeded.

Clipboard and tapped links are installed platform effects with the service
bodies in `proto/coax-native/clipboard-v1.json` and `open-link-v1.json` and
the one Coax codec `text-services-wire.coax`. Links pass the shared URI parser
and `text-link-policy.coax` in both application and installed host. Only
HTTP(S) documents open; no account headers are passed to them. Copy actions
can complete alongside a chat request without changing its turn state.

Tapping the conversation or blank area outside the composer dismisses the
keyboard. The recognizer preserves button/link actions and long-press text
selection; composer controls keep their normal keyboard behavior. Header
toolbars remain outside the scrolling transcript. Near the end, new content
and keyboard layout changes follow the end; scrolling back suspends that
follow until the reader reaches the end or sends another turn.

Build 1072 installs the semantic text renderer and these platform effects.
The message/state/parser code remains Coax and can update over the air within
this runtime. The existing whole-response chat transport, storage and model
admission are unchanged. This release does not add token streaming, T3's
work/tool log, math/Mermaid engines, Markdown image loading or new syntax
lexers. Code fences remain selectable monospace text with their language
and Copy control.
