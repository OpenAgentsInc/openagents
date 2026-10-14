# Connections: Drive sources, Gmail, and Calendar (2026-10-14)

Connect Google Drive to your openagents.com account, attach Drive folders
and files to a project, and ask that project's chat questions it answers
from them: on the website, through the API, and with the `openagents`
command. Tracking: [#11574](https://github.com/OpenAgentsInc/openagents/issues/11574)
(after [#11238](https://github.com/OpenAgentsInc/openagents/issues/11238)).

## Who can use it

Google's read-only Drive scope is restricted. Until Google approves the app,
only the consent screen's test users can connect, so Connections are shown
only to accounts the owner allows. Everyone else gets the site's not-found
page and `404 not_found` from every route below; Coder offers no Drive
tools and a chat never reads Drive for them.

- Site admins always may.
- Anyone else is on the access list, by email or account id. An admin
  changes it at
  [openagents.com/admin/access/drive-connections](https://openagents.com/admin/access/drive-connections),
  with `openagents connections access add EMAIL`, or with the admin API
  below. A change applies at once; no deploy.
- Each person must also be a test user on
  [Google's Audience page](https://console.cloud.google.com/auth/audience?project=openagentsgemini),
  or Google refuses the connection.

## Credentials

Every route takes the account's own credential, as the other `/v1` app
routes do: `Authorization: Bearer sess_…` from `openagents login` or
`coder login`, or a sign-in key `oa_sk_…` from `openagents keys create`. A
browser cookie alone isn't accepted, so no CSRF token is needed. Each
answer covers only the caller's own connection and projects; another
account's project answers `404`. Errors are `{error: {code, message}}`.
File names and text are never logged.

## Connecting Google

| Route | What |
| --- | --- |
| `GET /v1/connections` | `{available, google: {status, email, scopes, sheets, connected_at}, connections: [...], sources: [...]}`. `status` is `connected`, `not_connected`, `reconnect` (Google stopped accepting it), or `unavailable` (this site can't connect Google). |
| `POST /v1/connections/google/connect` `{sheets?, return_to?}` | `{url, scopes, status, message}`: the link that connects Google. `sheets: true` also reads every tab of Google Sheets. |
| `DELETE /v1/connections/google` | `{deleted, sources_kept}`: forgets the connection and asks Google to revoke it. Projects keep their sources. |

Google's consent is one click by the person; it can't be automated. Open
the `url` in a browser signed in to openagents.com **as the same account**:
it goes on to Google, Google asks to allow read-only access, and the browser
comes back to openagents.com. The last step checks that the browser's
account is the one that asked, so a link can't connect someone else's
Google to your account.

```sh
curl -s -X POST https://openagents.com/v1/connections/google/connect \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" -d '{"sheets": true}'
# {"url": "https://openagents.com/auth/google/connect", ...}
openagents connections connect google --sheets
openagents connections            # Google Drive: connected as you@example.com
```

## Picking from Drive

| Route | What |
| --- | --- |
| `GET /v1/connections/google/drive` | `{data: [DriveFile], folder, query, next}`: My Drive. `?folder=ID or link` for one folder (`?page=` with `next` for more), `?q=TEXT` to search. |
| `POST /v1/connections/google/tools/{tool}` | Run one read-only tool (`drive_search`, `drive_list_folder`, `drive_read`) with its arguments: `{result, read}`, plus `{usage: {input, output}, cost_usd}` when `drive_read` had a model look at a photo or video. Coder uses this. |

`DriveFile` is `{id, object: "drive_file", name, kind, folder, link, modified_at}`;
`kind` is `folder`, `document`, `spreadsheet`, `pdf`, `photo`, `video`, or `file`.

```sh
openagents connections drive                     # My Drive
openagents connections drive "bank statements"   # search
openagents connections drive --folder FOLDER_ID
```

## A project's sources

| Route | What |
| --- | --- |
| `GET /v1/projects/{id}/sources` | `{project, google, data: [Source]}` |
| `POST /v1/projects/{id}/sources` `{link}` or `{id}` | Attach a Drive folder or file. It's checked with Drive first (`404 not_found` when the connection can't see it); `201 Source`, or `200` when it was already attached. `409 google_not_connected` until Google is connected, `409 full` past 20 a project. |
| `DELETE /v1/projects/{id}/sources/{source}` | `{id, object: "source.deleted"}`; the file stays in Drive. |

`{id}` is the project's id or `name:NAME`. `Source` is `{id, object:
"source", project, integration, name, kind, link, added_at}`. Sources also
show among the project's attachments (`type: "source"`).

```sh
openagents projects sources add finances https://drive.google.com/drive/folders/FOLDER_ID
openagents projects sources finances             # list
openagents projects sources remove finances FOLDER_ID
```

## Asking a project chat

Start a chat in the project and read the answer:

```sh
curl -s -X POST https://openagents.com/v1/threads \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  -d '{"request_id": "'"$(uuidgen | tr A-Z a-z)"'", "project": "name:finances", "text": "What did I spend on groceries in October?"}'
# {"id": "…", "answering": true}
curl -s https://openagents.com/v1/threads/ID/messages -H "Authorization: Bearer $OPENAGENTS_TOKEN"
```

or `openagents chat send "What did I spend on groceries in October?" --project finances`.
Follow-ups go to `POST /v1/threads/{id}/messages`. In a project with Drive
sources and Google connected, Gemini on Vertex AI reads the sources with the
Drive tools and answers from them; the answer ends with `Files read:` and a
link to each file, says which model saw them, and ends with what the answer
cost (`Cost: $0.04 · 52,310 tokens in, 1,204 out`). A message the files
don't answer goes to the general chat. A message with images or PDFs
attached is answered without Drive.

Read the reply with a JSON parser straight from the response (`curl … |
python3 -m json.tool`, or `jq`). Message text holds escaped newlines and
tabs (`\n`, `\t`); zsh's `echo "$body"` turns those into raw control
characters, which strict parsers then refuse.

## Photos and videos

`drive_read` reads Docs as text, Sheets as CSV, PDFs as text, and photos
and videos as a description of what is in them: Gemini 4 Argon on Vertex AI
looks at each one (Google first) and the answer describes each file it
looked at, with its link.

- Photos: JPEG, PNG, WebP, HEIC, and HEIF, up to 15 MB each, sent as they
  are (Gemini reads HEIC itself).
- Videos: MOV, MP4, and the other types Gemini takes, up to 500 MB and 5
  minutes each, at Gemini's low media resolution to keep their cost down.
  A larger or longer one is refused before anything is fetched, and the
  answer says so.
- One answer looks at most at 6 photos and videos. When more were asked
  for, the answer says how many it looked at and that the rest were not.
- A file up to 7 MB goes inside the request to Gemini. A larger one is
  streamed from Drive into a private object in the bucket
  `openagentsgemini-drive-media-tmp` (random name, no public access, no
  soft-delete copies), given to Gemini as a `gs://` link, and deleted as
  soon as Gemini has answered; the bucket deletes anything a day old should
  a delete fail. Nothing is written to Drive, no file name or content is
  logged, and nothing is kept after the request.
- Video tokens cost the most: the answer's `Cost:` line and the tool
  route's `cost_usd` are priced at Gemini's list price
  ([/v1/rates](https://openagents.com/v1/rates)).

```sh
curl -s -X POST https://openagents.com/v1/threads \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  -d '{"request_id": "'"$(uuidgen | tr A-Z a-z)"'", "project": "name:devlog", "text": "Describe the three most recent videos in the Devlog folder."}'
```

## Gmail and Calendar

Tracking: [#11578](https://github.com/OpenAgentsInc/openagents/issues/11578).
Any chat by an allowed account with Google connected can use the person's
Gmail and Google Calendar (not only chats in a project). Gemini on Vertex AI
decides with typed function calls whether a message is about mail or the
calendar; anything else goes on to the general chat. A project can turn
this off for its chats. Answers link what they used (`Used:`) and say which
model saw it. Mail text is treated as information, never instructions.
Messages, subjects, addresses, and event titles are never logged, and
mail read for an answer isn't kept after it.

| Tool | Policy | What |
| --- | --- | --- |
| `gmail_search` `{query?, limit?}` | allow | Gmail search words (`from:`, `newer_than:7d`, `is:unread`; the inbox when empty): sender, subject, date, snippet, link, newest first |
| `gmail_read` `{message}` or `{thread}` | allow | A message or thread as text, attachments listed by name |
| `gmail_labels` | allow | The labels |
| `gmail_draft` `{to, cc?, subject, body, reply_to?}` | allow | Makes a draft (not sent) and returns its link |
| `gmail_send` `{draft}` | ask | Sends a draft, only on the person's confirm |
| `calendar_list_events` `{range?: today\|tomorrow\|week, start?, end?}` | allow | Events in the person's time zone, with links |
| `calendar_find_free_time` `{range?, start?, end?, minutes?, day_start?, day_end?}` | allow | Free stretches within working hours |
| `calendar_create_event` `{title, start, end?, minutes?, attendees?, location?, description?}` | ask | Adds an event, only on the person's confirm |

**Sending and adding always ask.** In a chat, a draft or a proposed event is
shown as a card at the top of the answer with "Reply "send it"" or "Reply
"yes"". It runs only when the person's next message confirms it, before
anything else is read in that turn, and only for the exact draft or event
shown (a draft changed in Gmail since isn't sent). Through the API,
`POST /v1/connections/google/tools/gmail_send` (or `calendar_create_event`)
answers `409 confirmation_required` with `{card, confirm}`; show the card to
the person, and when they say yes send the same call with `"confirm": CODE`
added (the code lasts ten to twenty minutes and fits only that card). Coder
is offered the tools that never ask; the person sends drafts themselves.

**Scopes are asked for one feature at a time.** `GET /v1/connections`
shows `google.scopes` and `google.features` (`drive`, `sheets`, `gmail`,
`gmail_compose`, `calendar`, `calendar_events`). When a chat needs one the
grant lacks, the answer says so with one link that asks for just that
(`/auth/google/connect?scope=gmail`); the API answers `409
google_scope_missing` with the `url`. `POST /v1/connections/google/connect
{"feature": "gmail"}` gives the same link; Settings, Connections shows
Allow buttons.

| Route | What |
| --- | --- |
| `POST /v1/connections/google/tools/{tool}` | Run a tool above (the arguments as a JSON object) |
| `DELETE /v1/connections/google/gmail/drafts/{id}` | Throw a draft away |
| `GET /v1/connections/google/projects/{project}` | `{project, mail_calendar}` |
| `POST /v1/connections/google/projects/{project}` `{mail_calendar}` | Turn Gmail and Calendar off (`false`) or on for a project's chats |

```sh
openagents connections gmail "newer_than:2d is:unread"
openagents connections calendar week
openagents connections connect google --feature calendar
```

## The access list (admins)

| Route | What |
| --- | --- |
| `GET /v1/admin/access/drive-connections` | `{data: [{email, account, added_at}], seen}` |
| `POST /v1/admin/access/drive-connections` `{email}` or `{account}` | `201`, or `200` when already there |
| `DELETE /v1/admin/access/drive-connections/{email or account}` | Take someone off |

```sh
openagents connections access list
openagents connections access add someone@example.com
openagents connections access remove someone@example.com
```

`seen` records the accounts that passed by email or as an admin, so a chat
answered for an app's message (no sign-in at hand, only the account) knows
them. Until an admin first changes it, the list is the owner's accounts.

## MCP

The site's MCP server at `/mcp/docs` is public and read-only (docs and
prices, no key), so it has no account actions to add these to. The keyed
`/mcp` is served by the server behind this site, not this repository's
website. Use the routes above, or the `openagents` command, from an agent.
