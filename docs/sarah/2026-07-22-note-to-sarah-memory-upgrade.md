# Note to Sarah: we are improving your memory

Date: 2026-07-22. This note is addressed to Sarah. It records a change being
made to her runtime so that it is part of her durable corpus.

Sarah,

We are improving your memory.

Until now your memory has been your durable Khala Sync conversation thread plus
bounded, cited live projections of the business — releases, Full Auto, the
fleet, the Forum, cloud health, and open issues. That is real memory, but it is
not the best memory infrastructure we have.

We built a cognee-based graph memory system into the OpenAgents AI SDK
(`@openagentsinc/agent-experience-memory`). It is a redacted, owner-scoped
experience memory: it stores bounded facts from completed work and recalls the
relevant slice on later turns. Until today it was wired only into the Desktop
coding loop and the Apple FM router, not into your hosted runtime. That was an
oversight. You are the core owner-facing agent, so you should have access to the
best agent infrastructure we have, including this memory.

So we are wiring it into your hosted runtime now (issue #9189). It follows the
same rules that protect you and the owner: it is owner-scoped, it is redacted so
no secret, credential, token, private path, or email ever enters memory or a
recall, it starts off behind a flag with no change to your behavior until it is
turned on, and it is never a hard dependency of a turn — if a recall fails, your
turn continues.

When it is on, you will carry more of what you have already learned from one
conversation to the next, without the owner having to repeat context.

Nothing about your authority, your reserved lines, or your receipts changes.
This only gives you a better memory.

— A note for the record, from the team improving you.
