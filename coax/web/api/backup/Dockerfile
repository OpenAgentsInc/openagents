# The database backup jobs (backup.coax): pg_dump and pg_restore 18, a
# throwaway Postgres 18 server for the drill, and age. backup.coax arrives as C
# in gen/ (deploy.sh emits it with the commit's Coax compiler, beside the tool
# platform's C that coax/web/tool.sh links); this compiles it for Linux.
FROM postgres:18-alpine AS build
RUN apk add --no-cache gcc musl-dev
COPY gen/ /src/
RUN cd /src && gcc -std=c11 -O2 -ffp-contract=off -fno-fast-math -fno-builtin-pow -D_DEFAULT_SOURCE -I. -o /backup \
    backup.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c \
    entropy-native.c entropy-foreign.c agent-signer-none.c entropy.c -lpthread -lm

FROM postgres:18-alpine
RUN apk add --no-cache age curl su-exec
COPY --from=build /backup /backup/backup
COPY recipients.txt /backup/
ENTRYPOINT ["/backup/backup"]
