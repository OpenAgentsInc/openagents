# Bring your own key

You can use your own provider key through the API. Requests on your key go
to your provider account, on your terms, and we charge nothing for them.

**Beta.**

## Use it

Add `"pay": "mine"` to the `openagents` object:

```json
{
  "model": "google/gemini-3.8-flash",
  "input": "Hi",
  "openagents": {"pay": "mine"}
}
```

With `"pay": "mine"`, only your own keys are used. If none of them can
serve the request, you get an error; we never fall back to our accounts
and charge you.

## What it costs

| | |
| --- | --- |
| Our fee | None during the beta |
| Your provider's charge | Billed to you by your provider |
| Usage | Still counted, so it shows with the rest of your usage |

## Your key's safety

Your key is stored sealed, used only for your own requests, and never shown
back in full. You can remove it at any time.

## When to use it

- You already have credit or a contract with a provider.
- You need a provider's own data terms to apply.
- You want a model we don't offer yet, through a provider you use.
