# Which model to use

Start with a task name such as `openagents/chat` and change it only when
you have a reason. Each one picks a model we've measured for that kind of
work, and switches providers if one fails before answering.

**Beta.**

## By task

| If you need | Use | Why |
| --- | --- | --- |
| Labels, routing, a yes or no | `openagents/classify` | Cheapest and fastest; small models do short labels well |
| A short, fast reply | `openagents/fast` | GLM-5.3 Flash: low price, quick first word |
| General chat | `openagents/chat` | Balanced price and quality |
| Coding with tools | `openagents/code`, or your own key | Picked by coding test scores |
| Very long input | `openagents/long` | Models that read a million tokens |
| Hard reasoning | `openagents/reason` | Larger models that think before answering |
| Not sure | `openagents/auto` | We read the request and pick one of the above |

## Price, speed, or quality

Every task name ranks models by quality first. To change the order, set
`openagents.route.sort`:

| You care most about | Set | What happens |
| --- | --- | --- |
| Quality | nothing | The best model for the task that meets its bar |
| Price | `"sort": "price"` | The cheapest model that is good enough |
| Speed | `"sort": "latency"` | The model that starts answering soonest |

```json
{
  "model": "openagents/chat",
  "input": "Summarize this ticket.",
  "openagents": {"route": {"sort": "price"}}
}
```

To cap what one request may cost, set `openagents.max_price` in dollars per
million tokens: `{"input": "0.50", "output": "1.50"}`. A model priced above
it is skipped.

## A specific model

Name it, such as `google/gemini-3.8-flash`, when you need the same model on
every request: for a test you compare over time, or a prompt tuned to one
model. You still get the cheapest provider for it unless you set
`route.order`.

## Your own key

Use your own provider key with `"pay": "mine"` when you have your own
contract, credit, or data terms with a provider. Your terms apply, and we
charge nothing on top. See [Bring your own key](/docs/api/bring-your-own-key).

## Community providers

People running open models on their own machines through Pylon will be able
to serve API requests at prices they set, paid in bitcoin. That comes after
the beta.

## Long conversations

The API keeps nothing between requests: send the whole conversation each
time. For very long histories, use `openagents/long`, or summarize older
turns yourself.
