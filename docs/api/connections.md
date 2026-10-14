# Connections and Drive sources (2026-10-14)

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
| `POST /v1/connections/google/tools/{tool}` | Run one read-only tool (`drive_search`, `drive_list_folder`, `drive_read`) with its arguments: `{result, read}`. Coder uses this. |

`DriveFile` is `{id, object: "drive_file", name, kind, folder, link, modified_at}`;
`kind` is `folder`, `document`, `spreadsheet`, `pdf`, or `file`.

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
link to each file, and says which model saw them. A message the files don't
answer goes to the general chat. A message with images or PDFs attached is
answered without Drive.

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
