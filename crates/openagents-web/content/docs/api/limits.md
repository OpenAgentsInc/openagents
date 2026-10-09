# Limits you set

We don't cap how much you use the API. You can set your own limits on your
account and on each key, and we hold every request to them.

**Beta.**

| Limit | What it stops |
| --- | --- |
| Spending cap | Spending more than an amount per day, week, or month |
| Price per request | Any model priced above your cap, per million tokens |
| Allowed models | Models you haven't allowed |
| Allowed providers | Providers you haven't allowed |
| Rate cap | More than a number of requests per minute |
| Expiry | Any use of the key after a date |

A request that hits one of your limits gets `403 limit_reached`, with
`param` naming the limit. Nothing else is charged for it.

For a single request, `openagents.max_price` caps its price without
changing your key's settings; see [Routing](/docs/api/routing).
