# Limits

Every limit we enforce on logging in. When you reach one, we say which and when it lifts.

| What | Limit |
| --- | --- |
| Email codes sent to one address | 5 an hour |
| Email codes sent to one address | 20 a day |
| Email codes sent from one network address | 30 an hour |
| Tries at one email code | 5 per code |
| Logins with a key or passkey from one network address | 30 a minute |
| App token refreshes for one login | 60 an hour |
| New accounts from one network address | 10 an hour |
| Agent and computer login requests from one network address | 30 an hour |
| Recovery requests for one account | 3 a week |
| Recovery code tries for one account | 5 an hour |
| Recovery code tries from one network address | 10 an hour |

This page is made from the table the API enforces (`coax/web/api/auth.c`).

## Lightning Addresses

On name@openagents.com. A wallet shows the limit's words when it refuses.

| What | Limit |
| --- | --- |
| Smallest payment | 1 sat |
| Largest payment | 1000000 sats (you can set it lower) |
| Note with a payment | 280 characters |
| Invoice expiry | 10 minutes |
| Lookups from one network address | 30 a minute |
| Invoice requests for one address | 120 a minute |
| Invoice requests from one network address | 30 a minute |
| Addresses | 1 per person |
| Wait before a new destination takes effect | 24 hours (72 for an account with only email) |

This section is made from the Lightning Address server's own numbers (`coax/web/api/lnaddr.c`).
