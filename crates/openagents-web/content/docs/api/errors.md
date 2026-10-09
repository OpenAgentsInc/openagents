# Errors

An error comes back as JSON with an HTTP status:

```json
{
  "error": {
    "type": "invalid_request",
    "code": "invalid_request",
    "param": "input",
    "message": "The request body isn't a valid request: ..."
  }
}
```

**Beta.**

| Status | `type` | What it means | What to do |
| --- | --- | --- | --- |
| 400 | `invalid_request` | The request is malformed, or asks for something the API doesn't do (such as `store: true`) | Fix the field named in `param` |
| 401 | `unauthorized` | The key is missing, wrong, or revoked | Check the `Authorization` header |
| 402 | `insufficient_balance` | Your balance can't cover the request | Top up, then retry |
| 403 | `limit_reached` | A limit you set was hit; `param` names it | Raise the limit, or wait for it to reset |
| 404 | `not_found` | No such model or route | Check the model name against `GET /v1/models` |
| 429 | `too_many_requests` | Too many requests at once | Retry after a short wait |
| 500 | `server_error`, `model_error` | Something broke on our side or the model's | Retry; tell us if it keeps happening |
| 502 | `upstream_failed` | Every provider we tried failed before answering | Retry, or name another model |
| 503 | `no_route` | No provider meets your request's needs, privacy setting, or price cap | Loosen `route`, `privacy`, or `max_price` |

Every answer has an `x-request-id` header. Include it when you ask us about
a request.

## Errors mid-answer

Once an answer has started, an error can't change the status code. A
stream ends with a `response.failed` event, and a full answer comes back
with `"status": "failed"`. Either way the `error` object carries a `code`
and a `message`.
