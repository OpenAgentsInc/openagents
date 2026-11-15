# Private Projects: API and mobile verification

The first consumer is `private-text-v1alpha1`, specified in
[the API contract](../protocol/projects-api-v1alpha1.md). It implements private
account-owned Projects, instructions, selected text/Markdown knowledge and personal
saved Project chats. New product terms and fields use **Worlds**. This is source
qualification, not a production API deployment or a TestFlight release.

API/integration base: `9312c968c7`. The Router changes on that base are preserved. Project turns
keep their Project context path; ordinary chats retain the Router flow.

| Check | Result |
| --- | --- |
| Private Project + history HTTP/DB checks after the Router merge | 29 + 65 passed; full selected API run passed in 438 seconds |
| Mobile Coax, `coax test apps/openagents-ios/home/app.coax --library` | 684 passed |
| iOS `CoaxNativeProjects`, `NativeProjectTests` and `NativeNavigationTests` | 11 passed on iPhone 17 Pro / iOS 26.5, headless simulator, simulator cloning disabled |
| Projects policy, `coax mutate coax/web/api/projects-policy.coax --library` | 65 mutants killed, zero survivors |
| Shared Projects codec | 131 passed; C ABI SHA-256 oracle also passed |
| Saved account chat codec and Project reply bounds | 168 passed |
| Protocol generation/check with protoc 29.3 / Python protobuf 5.29.3 | Passed, including published compatibility and module/profile checks |
| Projects JSON descriptor oracle | Every emitted object/list/error/context key holds to `projects.pb` |
| Wire, layer, tools and component checks | Passed; no allowlist increase |

The API suite runs remotely with Postgres, AddressSanitizer and UBSan. The broader
selection `API_TESTS_ONLY=test_inference.py,projects_checks.coax,history_checks.coax`
passed before the Router rebase (426 seconds): the new Project flow, existing
account/project history, streaming inference, usage/metering, credit and the
suite's common authorization, world and audit checks. Its inference adapter
source is unchanged by the rebase. A focused Project run also passed the legacy
`/api/projects/v1/files` dispatch case (a pre-existing Project may be named `v1`).

The Project HTTP checks cover two independent delivered-email logins, owner-only
read/write/list access, create replay and operation-content conflicts, stale
instruction saves, complete knowledge text reads, selected/unselected context,
saved thread association, funding, the captured outbound model request, reply
persistence before success, no redispatch under either the same or a new operation
ID for an admitted turn, and context hashes surviving knowledge removal.

Native checks exercise the installed Coax program and UIKit projection through
create, instruction edit, knowledge add, first and subsequent chat turns, saved
reply reads, structured attempt errors, retry and Stop waiting. Existing public
draft review, native upload SDK and navigation checks remain covered. Extra Coax
checks cover uncertain file knowledge saves, refused uploads staying private and
clearing private content after access is lost.

Bounds remain explicit: text/`.txt`/`.md`, 64 KiB per source, 32 KiB complete
instructions/reference context, the existing saved text transcript and history
window, existing upload grants and team beta inference funding. Sharing, imports,
PDF/Office extraction, rich blocks and other connections remain later consumers.
