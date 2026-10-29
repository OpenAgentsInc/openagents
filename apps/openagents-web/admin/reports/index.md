# Reports

Hecate's Cemetery's review queue: what visitors of [Planetary Villa](/projects/planetary-villa) reported (a memorial, a note on one, or a line in the chat), how many people reported it, what it said and why.

Each open report has HIDE and KEEP. HIDE takes the thing out of sight for everyone without deleting it; KEEP leaves it up and brings back a note or a chat line that three reports had held. The recent decisions are listed below the queue, and anything hidden there has RESTORE, which puts it back as it was. An owner can still delete their own pet for good.

Admins only, with the same login as [Analytics](/admin/analytics): `coax/web/api/admin.sh open` in the [OpenAgents repo](https://github.com/OpenAgentsInc/openagents). Anyone else sees nothing. The queue's rules are in `coax/web/api/migrations/0031_villa_reports.sql` and `0033_villa_review.sql`.
