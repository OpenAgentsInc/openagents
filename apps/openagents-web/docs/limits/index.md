# Limits

Every limit we enforce on logging in. When you reach one, we say which and when it lifts.

| What | Limit |
| --- | --- |
| Email codes sent to one address | 5 an hour |
| Email codes sent to one address | 20 a day |
| Email codes sent from one network address | 30 an hour |
| Tries at one email code | 5 per code |
| Logins with a key from one network address | 30 a minute |
| App token refreshes for one login | 60 an hour |
| New accounts from one network address | 10 an hour |

This page is made from the table the API enforces (`coax/web/api/auth.c`).
