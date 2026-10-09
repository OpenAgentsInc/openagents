# Privacy and data use

**Beta.**

## What we keep

We keep counts and timings for each request: which model and provider
answered, how many tokens, how long it took, and what it cost. We don't
keep your prompts or the answers.

## Where your request goes

You choose with `openagents.privacy`:

| Setting | Your request goes only to providers that |
| --- | --- |
| `strict` (the default) | Don't train on it and don't keep it |
| `standard` | Don't train on it, but may keep it for a short time to check for abuse |

Each provider in `GET /v1/models` says whether it keeps nothing
(`zero_retention`). A provider whose terms we haven't checked never gets
`strict` requests. Today that's Z.ai and OpenAgents Pro, which serve
`standard` requests only.

## Your own key

With `"pay": "mine"`, your request goes to your own provider account, under
your own terms with that provider. See
[Bring your own key](/docs/api/bring-your-own-key).

## More

The [Privacy Policy](/privacy) covers everything else about your data at
OpenAgents.
