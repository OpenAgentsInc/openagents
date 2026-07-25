# OpenAgents analytics

This package owns the portable first-party analytics contract. It contains no
browser, mobile, database, or authentication APIs.

Clients can record only the event names in `AnalyticsEventName`. Add a name to
that closed set before a product flow uses it. The client queues events and
sends bounded batches through an injected Effect transport.

```ts
const layer = makeAnalyticsClientLayer(
  {
    send: (batch) => postBatchWithThePlatformTransport(batch),
  },
  {
    nextId: () => Effect.sync(makePlatformEventId),
    nowIso: () => Effect.sync(() => new Date().toISOString()),
  },
  {
    client: "mobile",
    maxBatchSize: 10,
    maxBufferedEvents: 100,
  },
);

const runtime = ManagedRuntime.make(layer);
runtime.runFork(
  Effect.flatMap(AnalyticsClient, (client) => client.track("github_view", "/")),
);
```

The platform adapter owns asynchronous startup, lifecycle flushes, and network
delivery. The server owns validation, trusted receipt time, idempotency,
retention, aggregate reads, and admin authorization.

## Web operation

The website enables analytics automatically in development. Production needs
both of these settings:

- Set `VITE_WEB_ANALYTICS_ENABLED=true` in the website build environment.
- Set `WEB_ANALYTICS_ENABLED=true` on the Cloud Run service.

The committed staging and production Cloud Run environment files enable the
server switch. Apply migration `0097_web_analytics_events.sql` before you
deploy this setting. Set both switches to `false` to stop collection.

The same-origin ingest route is `POST /api/analytics/events`. The admin summary
route is `GET /api/admin/web-analytics`. The dashboard is at
`/admin/analytics`. It uses the existing OpenAuth browser session and the
existing OpenAgents admin email allowlist.
