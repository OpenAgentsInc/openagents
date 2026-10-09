# Models and prices

**Beta.** Prices on this page are the ones the API charges.

## Prices

{{rate card}}

The same card is JSON at `GET /v1/rates`, open without a key. Each model in
`GET /v1/models` carries its rows too.

A few rules:

- **The list price, whoever answers.** When a model has more than one
  provider, you pay that provider's row. Our own deals with providers never
  change the price you see.
- **Promotions are their own rows**, labeled, beside the list price.
- **Your own key costs nothing extra.** See
  [Bring your own key](/docs/api/bring-your-own-key).

## What each model can do

{{model table}}

## Model names

Model names are `publisher/model`, such as `zai/glm-5.3-flash`. Names that
start with `openagents/` pick a model for you by task:

| Name | For |
| --- | --- |
| `openagents/auto` | We read the request and pick the task below |
| `openagents/classify` | Labels, extraction, a short yes or no |
| `openagents/fast` | Short replies, openers, summaries |
| `openagents/chat` | General conversation |
| `openagents/code` | Coding and tool use |
| `openagents/long` | Inputs over 200,000 tokens |
| `openagents/reason` | Hard multi-step problems |

[Which model to use](/docs/api/decisions) helps you choose.
