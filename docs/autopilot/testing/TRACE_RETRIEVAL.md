# Trace Retrieval (Rust Control + Runtime)

Use this runbook to retrieve request traces across control/runtime services.

## Inputs

1. `x-request-id` from HTTP response headers.
2. Time window and target service.

## Cloud Run log lookup

Control (prod):
```bash
gcloud logging read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name="openagents-web" AND textPayload:"<REQUEST_ID>"' \
  --project openagentsgemini --limit 200 --format json
```

Control (staging):
```bash
gcloud logging read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name="openagents-web-staging" AND textPayload:"<REQUEST_ID>"' \
  --project openagentsgemini --limit 200 --format json
```

Runtime:
```bash
gcloud logging read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name="openagents-runtime" AND textPayload:"<REQUEST_ID>"' \
  --project openagentsgemini --limit 200 --format json
```

## Notes

1. Khala websocket events should be correlated by topic + seq + request id where available.
2. Persist investigation artifacts under `docs/reports/` when used for release decisions.
