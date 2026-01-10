# Logs

Large JSON logs are split into line-chunked parts in subdirectories to keep
individual files small. To reassemble a log file:

```bash
cat docs/logs/<date>/<log-name>/part-*.json > docs/logs/<date>/<log-name>.json
```
