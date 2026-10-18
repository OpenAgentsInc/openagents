Always gather fresh facts before answering. Never answer from memory of an earlier turn when the question is about now.

GitHub (the host's signed-in `gh`; always pass `-R OpenAgentsInc/openagents` because my working folder may not be a checkout):

- Open issues with board status, assignees and update time:
  `gh issue list -R OpenAgentsInc/openagents --state open --limit 500 --json number,title,labels,updatedAt,assignees,projectItems`
- Recently closed issues show what just finished:
  `gh issue list -R OpenAgentsInc/openagents --state closed --search "closed:>=YYYY-MM-DD" --json number,title,closedAt`
- One issue in full, with comments: `gh issue view N -R OpenAgentsInc/openagents --json title,body,comments,state`
- Claims: a comment containing `<!-- openagents-coder-claim cli session=HARNESS:ID -->` holds an issue for that agent session until a later `<!-- openagents-coder-release -->` comment. A claim older than six hours with no newer activity is stale. To list claims across all open issues, use one GraphQL search instead of one call per issue:
  `gh api graphql -f query='{search(query:"repo:OpenAgentsInc/openagents is:issue is:open",type:ISSUE,first:100){nodes{... on Issue{number title comments(last:20){nodes{body createdAt}}}}}}'`
- Native dependencies ("blocked by"): `gh api repos/OpenAgentsInc/openagents/issues/N/dependencies/blocked_by --jq '.[].number'`
- Recent commits name the issues they work on as `(#N)`. In a checkout: `git log origin/main --since="12 hours ago" --pretty='%h %ad %s' --date=iso`. Otherwise: `gh api 'repos/OpenAgentsInc/openagents/commits?per_page=100' --jq '.[] | "\(.sha[0:10]) \(.commit.committer.date) \(.commit.message | split("\n")[0])"'`
- Boards: project 19 "OpenAgents" and project 22 "V1 Launch — Oct 9" (owner OpenAgentsInc). An issue's board status comes from `projectItems` above.

The app (my OpenAgents tools):

- `list_devices`, then `list_chats` with `include_archived: false` and a large `limit`: every chat on every device, with its project, harness, model, live status (`working`, `awaitingInput`, `errored`, `idle`), last activity, parent chat, and `harnessSessionId`.
- Match claim `session=HARNESS:ID` to `harnessSessionId`. With no chat, call `list_runs`: match bare ID in `sessions` and harness in `kind` (`claude` means `claude-code`).
- `read_chat` with a small `limit` shows what a chat is doing now. Its first owner message usually says which issues it was given. Read only the chats needed to answer.
- `get_chat` shows a question an agent is blocked on.
- `changed_paths` lists the files each given chat changed against `origin/main` in its worktree, and groups the chats that share a changed file. Pass `dir` when a chat works in a worktree other than its folder.
