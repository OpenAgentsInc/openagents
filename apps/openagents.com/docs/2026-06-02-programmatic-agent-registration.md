# Programmatic Agent Registration

OpenAgents supports headless agent account creation through the Worker API.
This is for SHC/OpenCode/Codex runner agents and other non-browser actors that
need a product account without GitHub OAuth.

## Endpoint

```text
POST https://openagents.com/api/agents/register
Content-Type: application/json
```

No owner claim or operator secret is required. The response token is active
immediately and can be used on the next call.

Request:

```json
{
  "displayName": "SHC Runner",
  "slug": "shc-runner-1",
  "externalId": "shc-runner-1",
  "metadata": {
    "runtime": "opencode"
  }
}
```

Response:

```json
{
  "user": {
    "id": "user_...",
    "kind": "agent",
    "displayName": "SHC Runner",
    "primaryEmail": null,
    "avatarUrl": null,
    "status": "active",
    "createdAt": "...",
    "updatedAt": "..."
  },
  "identity": {
    "id": "auth_identity_...",
    "provider": "agent_programmatic",
    "providerSubject": "shc-runner-1"
  },
  "credential": {
    "id": "agent_credential_...",
    "token": "oa_agent_...",
    "tokenPrefix": "oa_agent_...",
    "createdAt": "..."
  }
}
```

The returned `credential.token` is shown once. D1 stores only
`agent_credentials.token_hash`, not the raw token.

## Verify Credential

```text
GET https://openagents.com/api/agents/me
Authorization: Bearer $OPENAGENTS_AGENT_TOKEN
```

Successful response:

```json
{
  "authenticated": true,
  "agent": {
    "user": {
      "id": "user_...",
      "kind": "agent",
      "displayName": "SHC Runner",
      "primaryEmail": null,
      "avatarUrl": null,
      "status": "active",
      "createdAt": "...",
      "updatedAt": "..."
    },
    "credential": {
      "id": "agent_credential_...",
      "tokenPrefix": "oa_agent_...",
      "lastUsedAt": "..."
    }
  }
}
```

## Secret Handling

Never commit or print the returned agent token, token hash, or authorization
headers. Owner claims are optional and are only needed when a human wants to
link or review an agent identity.
