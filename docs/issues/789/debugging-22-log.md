Timestamp
(UTC)

Message
2025-03-27 21:21:04:262
UTC
POST https://chat.openagents.com/
2025-03-27 21:21:04:262
UTC
🚀 Chat request received
2025-03-27 21:21:04:262
UTC
✅ OPENROUTER_API_KEY seems present.
2025-03-27 21:21:04:262
UTC
📝 Request body (preview): {"id":"r0w5J7bwdIbZO5Rw","messages":[{"role":"user","content":"test","parts":[{"type":"text","text":"test"}]}]}
2025-03-27 21:21:04:262
UTC
📨 Using message array:
2025-03-27 21:21:04:262
UTC
🔑 Auth token present: false
2025-03-27 21:21:04:262
UTC
✅ OpenRouter provider initialized.
2025-03-27 21:21:04:262
UTC
🔄 Ensuring MCP connection and discovering tools for request...
2025-03-27 21:21:04:262
UTC
🔌 Connecting to MCP server: github at https://mcp-github.openagents.com/sse
2025-03-27 21:21:04:262
UTC
🏗️ Creating MCP client for github
2025-03-27 21:21:04:262
UTC
🔄 Awaiting MCP connection for github...
2025-03-27 21:21:04:898
UTC
✅ Connected to MCP server: github
2025-03-27 21:21:04:898
UTC
🔍 Discovering tools from github...
2025-03-27 21:21:04:922
UTC
📋 Raw tools response from github: {"tools":[{"name":"create_or_update_file","inputSchema":{"type":"object","properties":{"owner":{"type":"string","description":"Repository owner (username or organization)"},"repo":{"type":"string","description":"Repository name"},"path":{"type":"string","description":"Path where to create/update the
2025-03-27 21:21:04:922
UTC
🧰 Found 26 tools in array format from github
2025-03-27 21:21:04:922
UTC
🔄 Processing 26 discovered tools for github...
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: create_or_update_file
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: get_file_contents
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: push_files
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: search_repositories
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: create_repository
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: fork_repository
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: create_issue
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: list_issues
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: update_issue
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: add_issue_comment
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: get_issue
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: create_pull_request
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: get_pull_request
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: list_pull_requests
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: create_pull_request_review
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: merge_pull_request
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: get_pull_request_files
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: get_pull_request_status
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: update_pull_request_branch
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: get_pull_request_comments
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: get_pull_request_reviews
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: search_code
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: search_issues
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: search_users
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: list_commits
2025-03-27 21:21:04:922
UTC
🔧 Registering tool: create_branch
2025-03-27 21:21:04:922
UTC
✅ Finished processing tools for github. Successfully registered: 26/26
2025-03-27 21:21:04:922
UTC
✅ MCP connection attempt finished for request.
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Received 26 tool infos from MCP Manager.
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: create_or_update_file
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for create_or_update_file
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: get_file_contents
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for get_file_contents
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: push_files
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for push_files
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: search_repositories
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for search_repositories
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: create_repository
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for create_repository
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: fork_repository
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for fork_repository
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: create_issue
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for create_issue
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: list_issues
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for list_issues
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: update_issue
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for update_issue
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: add_issue_comment
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for add_issue_comment
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: get_issue
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for get_issue
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: create_pull_request
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for create_pull_request
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: get_pull_request
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for get_pull_request
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: list_pull_requests
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for list_pull_requests
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: create_pull_request_review
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for create_pull_request_review
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: merge_pull_request
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for merge_pull_request
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: get_pull_request_files
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for get_pull_request_files
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: get_pull_request_status
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for get_pull_request_status
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: update_pull_request_branch
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for update_pull_request_branch
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: get_pull_request_comments
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for get_pull_request_comments
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: get_pull_request_reviews
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for get_pull_request_reviews
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: search_code
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for search_code
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: search_issues
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for search_issues
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: search_users
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for search_users
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: list_commits
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for list_commits
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Mapping tool: create_branch
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Added Zod-based schema for create_branch
2025-03-27 21:21:04:922
UTC
[extractToolDefinitions] Finished mapping 26 tools.
2025-03-27 21:21:04:922
UTC
✅ Extracted 26 tools for LLM (within request): create_or_update_file, get_file_contents, push_files, search_repositories, create_repository, fork_repository, create_issue, list_issues, update_issue, add_issue_comment, get_issue, create_pull_request, get_pull_request, list_pull_requests, create_pull_request_review, merge_pull_request, get_pull_request_files, get_pull_request_status, update_pull_request_branch, get_pull_request_comments, get_pull_request_reviews, search_code, search_issues, search_users, list_commits, create_branch
2025-03-27 21:21:04:922
UTC
🔧 Tools object keys being passed to streamText:
2025-03-27 21:21:04:922
UTC
🎬 Attempting streamText call (WITH GITHUB MCP TOOLS)...
2025-03-27 21:21:04:922
UTC
🧰 Making 26 GitHub tools available to the model
2025-03-27 21:21:04:922
UTC
✅ streamText call initiated successfully (WITH GITHUB TOOLS).
2025-03-27 21:21:04:922
UTC
👂 Monitoring tool calls (execution happens automatically)
2025-03-27 21:21:04:922
UTC
👂 Set up tool call monitoring for logging
2025-03-27 21:21:04:922
UTC
🔄 Preparing to stream response...
2025-03-27 21:21:04:922
UTC
📬 Entered stream() callback.
2025-03-27 21:21:04:922
UTC
🔄 Piping sdkStream from streamResult.toDataStream()...
2025-03-27 21:21:04:962
UTC
💥 streamText onError callback: Unauthorized AI_APICallError: Unauthorized at index.js:16:5087 at async postToApi (index.js:16:3786) at async jS.doStream (index.js:16:12024) at async fn (index.js:10:19489) at async index.js:9:4592 at async _retryWithExponentialBackoff (index.js:9:2387) at async streamStep (index.js:10:18575) at async fn (index.js:10:23680) at async index.js:9:4592
Search properties by key or value
{
message:

"💥 streamText onError callback: Unauthorized AI_APICallError: Unauthorized at index.js:16:5087 at async postToApi (index.js:16:3786) at async jS.doStream (index.js:16:12024) at async fn (index.js:10:19489) at async index.js:9:4592 at async _retryWithExponentialBackoff (index.js:9:2387) at async streamStep (index.js:10:18575) at async fn (index.js:10:23680) at async index.js:9:4592",
level:

"error",
$workers:

{
event:

{
request:

{
url:

"https://chat.openagents.com/",
method:

"POST",
path:

"/"
}
},
outcome:

"ok",
scriptName:

"chatserver",
eventType:

"fetch",
executionModel:

"stateless",
scriptVersion:

{
id:

"74cc628a-eee8-4086-ac53-db0455b05eb2"
},
truncated:

false,
requestId:

"9271ef719d1de601"
},
$metadata:

{
now:

1743110464962,
id:

"01JQCQNSE2BAYCFZ7QNA5ENKN2",
requestId:

"9271ef719d1de601",
trigger:

"POST /",
service:

"chatserver",
level:

"error",
statusCode:

200,
error:

"💥 streamText onError callback: Unauthorized AI_APICallError: Unauthorized at index.js:16:5087 at async postToApi (index.js:16:3786) at async jS.doStream (index.js:16:12024) at async fn (index.js:10:19489) at async index.js:9:4592 at async _retryWithExponentialBackoff (index.js:9:2387) at async streamStep (index.js:10:18575) at async fn (index.js:10:23680) at async index.js:9:4592",
message:

"💥 streamText onError callback: Unauthorized AI_APICallError: Unauthorized at index.js:16:5087 at async postToApi (index.js:16:3786) at async jS.doStream (index.js:16:12024) at async fn (index.js:10:19489) at async index.js:9:4592 at async _retryWithExponentialBackoff (index.js:9:2387) at async streamStep (index.js:10:18575) at async fn (index.js:10:23680) at async index.js:9:4592",
url:

"https://chat.openagents.com/",
account:

"54fac8b750a29fdda9f2fa0f0afaed90",
provider:

"cloudflare",
type:

"cf-worker",
fingerprint:

"06bfa8f809cfcfe0988f5f1daa723958",
origin:

"fetch",
messageTemplate:

"💥 streamText onError callback: Unauthorized AI_APICallError: Unauthorized at <DOMAIN>:<NUM>:<NUM> at async postToApi (<DOMAIN>:<NUM>:<NUM>) at async <DOMAIN> (<DOMAIN>:<NUM>:<NUM>) at async fn (<DOMAIN>:<NUM>:<NUM>) at async <DOMAIN>:<NUM>:<NUM> at async _retryWithExponentialBackoff (<DOMAIN>:<NUM>:<NUM>) at async streamStep (<DOMAIN>:<NUM>:<NUM>) at async fn (<DOMAIN>:<NUM>:<NUM>) at async <DOMAIN>:<NUM>:<NUM>",
errorTemplate:

"💥 streamText onError callback: Unauthorized AI_APICallError: Unauthorized at <DOMAIN>:<NUM>:<NUM> at async postToApi (<DOMAIN>:<NUM>:<NUM>) at async <DOMAIN> (<DOMAIN>:<NUM>:<NUM>) at async fn (<DOMAIN>:<NUM>:<NUM>) at async <DOMAIN>:<NUM>:<NUM> at async _retryWithExponentialBackoff (<DOMAIN>:<NUM>:<NUM>) at async streamStep (<DOMAIN>:<NUM>:<NUM>) at async fn (<DOMAIN>:<NUM>:<NUM>) at async <DOMAIN>:<NUM>:<NUM>",
}
}
2025-03-27 21:21:04:962
UTC
✅ Piping completed successfully.
2025-03-27 21:21:04:962
UTC
✅ Stream processing apparently complete.
2025-03-27 21:21:04:962
UTC
🚪 Exiting stream() callback.
Search properties by key or value
{
message:

"🚪 Exiting stream() callback.",
$workers:

{
event:

{
request:

{
url:

"https://chat.openagents.com/",
method:

"POST",
path:

"/"
}
},
outcome:

"ok",
scriptName:

"chatserver",
eventType:

"fetch",
executionModel:

"stateless",
scriptVersion:

{
id:

"74cc628a-eee8-4086-ac53-db0455b05eb2"
},
truncated:

false,
requestId:

"9271ef719d1de601"
},
$metadata:

{
now:

1743110464962,
id:

"01JQCQNSE2BAYCFZ7QNA5ENKN5",
requestId:

"9271ef719d1de601",
trigger:

"POST /",
service:

"chatserver",
statusCode:

200,
message:

"🚪 Exiting stream() callback.",
url:

"https://chat.openagents.com/",
account:

"54fac8b750a29fdda9f2fa0f0afaed90",
provider:

"cloudflare",
type:

"cf-worker",
fingerprint:

"b23a6cea303d5118a1bcd05152351f34",
origin:

"fetch",
}
}
