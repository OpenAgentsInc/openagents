# `results_trend`

```json
{
  "schema": "openagents.gym.results_trend.v1",
  "suite_id": "tb2-quick",
  "suite_key": "suite:abc123abc123abc123abc123abc123abc123abc123abc123abc123abc123",
  "verified": true,
  "lane_comparisons": [
    {
      "suite_key": "suite:abc123abc123abc123abc123abc123abc123abc123abc123abc123abc123",
      "suite_id": "tb2-quick",
      "tasks": [
        "regex-log",
        "openssl-selfsigned-cert"
      ],
      "baseline_lane": "proxy",
      "lanes": [
        {
          "lane": "proxy",
          "run_digest": "run:abc123abc123abc123abc123abc123abc123",
          "recorded_at": "2026-08-26T12:00:00Z",
          "cost_per_accepted_outcome_usd": 0.75,
          "success_rate": 1.0,
          "cost_delta": null,
          "success_rate_delta": null
        },
        {
          "lane": "local",
          "run_digest": "run:def456def456def456def456def456def456",
          "recorded_at": "2026-08-27T12:00:00Z",
          "cost_per_accepted_outcome_usd": null,
          "success_rate": 0.5,
          "cost_delta": {
            "from": 0.75,
            "direction": "unpriced",
            "reason": "no cost delta: the local lane reports cost_unknown"
          },
          "success_rate_delta": {
            "from": 1.0,
            "to": 0.5,
            "absolute": -0.5,
            "relative": -0.5,
            "direction": "worse",
            "reason": "success rate fell"
          }
        }
      ],
      "confounders": [
        "model also varies (openai/gpt-5.6-luna, local/gpt-5.6-luna)"
      ]
    }
  ],
  "trends": [
    {
      "suite_key": "suite:abc123abc123abc123abc123abc123abc123abc123abc123abc123abc123",
      "suite_id": "tb2-quick",
      "lane": "proxy",
      "steps": [
        {
          "from_recorded_at": "2026-08-25T12:00:00Z",
          "to_recorded_at": "2026-08-26T12:00:00Z",
          "cost_delta": {
            "from": 0.85,
            "to": 0.75,
            "absolute": -0.1,
            "relative": -0.11764705882352941,
            "direction": "better",
            "reason": "cost per accepted outcome fell"
          },
          "success_rate_delta": {
            "from": 0.9,
            "to": 1.0,
            "absolute": 0.1,
            "relative": 0.1111111111111111,
            "direction": "better",
            "reason": "success rate rose"
          },
          "confounders": []
        }
      ]
    }
  ],
  "isolated_groups": 0
}
```
