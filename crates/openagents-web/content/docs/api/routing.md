# Routing and credits

Every request names a model or a task. We turn that into at most three
tries, best first, and answer with the first one that starts writing.

**Beta.**

## How we pick

1. **Who can serve it.** A model name means every provider that offers that
   model. A task name, such as `openagents/chat`, means its list of models.
2. **Who can't.** We drop a provider that can't do what your request needs
   (tools, structured output, images, its length), doesn't meet your privacy
   setting, costs more than your `max_price`, or is failing right now.
3. **Good enough.** For task names, a model must pass that task's quality
   bar, measured daily.
4. **Order.** Then by our prepaid credit first, then price, then how fast
   it starts answering. Your price is the same whichever one answers.
5. **Try.** If a provider fails before the first word, we try the next one.
   After the first word, a failure ends the answer with `response.failed`.

The answer's `openagents` object lists each try.

## Your options

All are optional, in an `openagents` object in the request body:

```json
{
  "model": "openagents/chat",
  "input": "Hi",
  "openagents": {
    "route": {"sort": "price", "ignore": ["openrouter"]},
    "privacy": "strict",
    "pay": "ours",
    "max_price": {"input": "0.50", "output": "1.50"},
    "fallbacks": ["zai/glm-5.3-flash"]
  }
}
```

| Option | What it does |
| --- | --- |
| `route.sort` | `quality` (the default), `price`, or `latency` |
| `route.order` | Providers to try first, in order |
| `route.only` | Use only these providers |
| `route.ignore` | Never use these providers |
| `privacy` | `strict` (the default) or `standard`; see [Privacy](/docs/api/privacy) |
| `pay` | `ours` (the default) or `mine` for [your own key](/docs/api/bring-your-own-key) |
| `max_price` | The most you'll pay, dollars per million tokens, margin included |
| `fallbacks` | Other models to try if yours can't answer |

Providers are named by their short ids: `vertex` (Google Vertex AI), `zai`
(Z.ai), `pro` (OpenAgents Pro), `openrouter`, and `vercel` (Vercel AI
Gateway). `GET /v1/models` lists which serve each model.

## Our credit and your price

We hold prepaid credit with some providers and spend it first, but only on
models that pass the task's quality bar. Credit lowers our cost, never your
price: you always pay the list price plus our margin on the
[rate card](/docs/api/models).
