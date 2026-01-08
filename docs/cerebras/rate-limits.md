# Rate Limits

> Learn how rate limits are applied and measured.

Rate limits ensure fair usage and system stability by regulating how often users and applications can access our API within a specified timeframe. They help protect our service from abuse or misuse and keep your access fair and without slowdowns.

## How are rate limits measured?

We measure rate limits in requests sent and tokens used within a specified timeframe:

* Requests per minute/hour/day (RPM, RPH, RPD)
* Tokens per minute/hour/day (TPM, TPH, TPD)

Rate limiting can be triggered by any metric, whichever comes first. For example, you have a rate limit of 50 RPM and 200K TPM. If you submit 50 requests in one minute with just 100 tokens each, you'll hit your limit even though your total token usage (5,000) is far below the 200K token threshold.

Rate limits apply at the organization level, not the user level, and vary based on the model.

### Token Rate Limiting

When you send a request, we estimate the total tokens that will be consumed by:

1. Estimating the input tokens in your prompt
2. Adding either the `max_completion_tokens` parameter or the maximum sequence length (MSL), minus input tokens

If this estimated token consumption would exceed your available token quota, the request is rate limited before processing begins. This ensure fair usage and system stability.

**Best practice**: Set [`max_completion_tokens`](/api-reference/chat-completions#param-max-completion-tokens) appropriately for your use case to avoid overestimating token usage and triggering unnecessary rate limits.

### Quota Replenishment

Your quota is calculated as:

```
Available quota = Rate limit - Usage in current time window
```

We use the [token bucketing](https://en.wikipedia.org/wiki/Token_bucket) algorithm for rate limiting, which means your capacity replenishes continuously rather than resetting at fixed intervals. As you consume tokens or requests, your available capacity automatically refills up to your maximum limit.

This token bucketing approach ensures smoother API access and prevents the "burst at interval start, then idle" pattern.

## Limits by Tier

This provides an overview of general limits, though specific cases may vary. For precise, up-to-date rate limit information applicable to your organization, check the Limits section within your account.

<Tabs>
  <Tab title="Free">
    | Model                            | TPM  | TPH | TPD | RPM | RPH | RPD   |
    | -------------------------------- | ---- | --- | --- | --- | --- | ----- |
    | `gpt-oss-120b`                   | 60K  | 1M  | 1M  | 30  | 900 | 14.4K |
    | `llama3.1-8b`                    | 60K  | 1M  | 1M  | 30  | 900 | 14.4K |
    | `llama-3.3-70b`                  | 60K  | 1M  | 1M  | 30  | 900 | 14.4K |
    | `qwen-3-32b`                     | 60K  | 1M  | 1M  | 30  | 900 | 14.4K |
    | `qwen-3-235b-a22b-instruct-2507` | 60K  | 1M  | 1M  | 30  | 900 | 14.4K |
    | `zai-glm-4.6`                    | 150K | 1M  | 1M  | 10  | 100 | 100   |
  </Tab>

  <Tab title="Developer">
    | Model                            | TPM  | RPM |
    | -------------------------------- | ---- | --- |
    | `gpt-oss-120b`                   | 1M   | 1K  |
    | `llama3.1-8b`                    | 1M   | 1K  |
    | `llama-3.3-70b`                  | 1M   | 1K  |
    | `qwen-3-32b`                     | 1M   | 1K  |
    | `qwen-3-235b-a22b-instruct-2507` | 1M   | 1K  |
    | `zai-glm-4.6`                    | 250K | 250 |

    <Note>Hourly and daily restrictions don't apply to developer tier users. Since this tier uses pay-as-you-go pricing, you can use as many tokens as needed within your budget.</Note>
  </Tab>
</Tabs>

## Rate Limit Headers

To help you monitor your usage in real time, we inject several custom headers into every API response. These headers provide insight into your current usage and when your limits will reset.

You’ll find the following headers in the response:

| Header                                | Description                                                 |
| ------------------------------------- | ----------------------------------------------------------- |
| `x-ratelimit-limit-requests-day`      | Maximum number of requests allowed per day.                 |
| `x-ratelimit-limit-tokens-minute`     | Maximum number of tokens allowed per minute.                |
| `x-ratelimit-remaining-requests-day`  | Number of requests remaining for the current day.           |
| `x-ratelimit-remaining-tokens-minute` | Number of tokens remaining for the current minute.          |
| `x-ratelimit-reset-requests-day`      | Time (in seconds) until your daily request limit resets.    |
| `x-ratelimit-reset-tokens-minute`     | Time (in seconds) until your per-minute token limit resets. |

These values update with each API call, giving you immediate visibility into your current usage.

### Example

You can view these headers by adding the `--verbose` flag to a cURL request:

```bash  theme={null}
curl --location 'https://api.cerebras.ai/v1/chat/completions' \
--header 'Content-Type: application/json' \
--header "Authorization: Bearer ${CEREBRAS_API_KEY}" \
--data '{
  "model": "llama3.1-8b",
  "stream": false,
  "messages": [{"content": "Hello!", "role": "user"}],
  "temperature": 0,
  "max_completion_tokens": -1,
  "seed": 0,
  "top_p": 1
}' \
--verbose
```

In the response, look for headers like these:

```
x-ratelimit-limit-requests-day: 1000000000
x-ratelimit-limit-tokens-minute: 1000000000
x-ratelimit-remaining-requests-day: 999997455
x-ratelimit-remaining-tokens-minute: 999998298
x-ratelimit-reset-requests-day: 33011.382867097855
x-ratelimit-reset-tokens-minute: 11.382867097854614
```

## Notes

<Note>If you exceed your rate limits, you will receive a [429 Too Many Requests error](/support/error).</Note>

If you have questions about your usage or need higher rate limits, [contact us](https://www.cerebras.ai/contact) via our website, or reach out to your account representative.


---

> To find navigation and other pages in this documentation, fetch the llms.txt file at: https://inference-docs.cerebras.ai/llms.txt
