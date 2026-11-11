# The database backup jobs (backup.sh): pg_dump and pg_restore 18, a
# throwaway Postgres 18 server for the drill, and age.
FROM postgres:18-alpine
RUN apk add --no-cache age bash coreutils curl su-exec
COPY backup.sh recipients.txt /backup/
ENTRYPOINT ["/backup/backup.sh"]
