# Config

Version 2 of the config file looks like this:

```json
{"version": 2, "database": {"host": "db.internal", "port": 5432}, "debug": true}
```

- `database.port` is an integer.
- `debug` is a boolean: version 1's `"yes"`/`"no"` become `true`/`false`.
