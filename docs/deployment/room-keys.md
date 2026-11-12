# Room keys: rollout and rotation

This runbook moves Everglade's room shards off the one shared seat-ticket key
(`oa-room-ticket-key`) and onto two kinds of key (#13260):

- **Ticket keys.** The API signs seat tickets (version 5,
  [seat-ticket-v5](../protocol/seat-ticket-v5.md)) with a private key. Rooms
  check them with its public key, so a room can't mint a ticket or an admin
  seat.
- **Write keys.** Each shard signs what it writes to the API with its own key
  ([room-write-v1](../protocol/room-write-v1.md)), so one shard's key can't
  write as another shard.

Nothing changes until you create the new secrets. Each deploy script uses a
secret only when it exists:

| Secret | Holder | Variable | Deploy script |
| --- | --- | --- | --- |
| `oa-room-ticket-signing-key` | API | `OA_ROOM_TICKET_SIGNING_KEY` | `coax/web/api/deploy.sh` |
| `oa-room-write-master` | API | `OA_ROOM_WRITE_MASTER` | `coax/web/api/deploy.sh` |
| `oa-room-ticket-pubkeys` | every shard | `OA_ROOM_TICKET_PUBKEYS` | `coax/web/room/deploy.sh`, `vm-deploy.sh` |
| `oa-room-write-key-sK` | shard K only | `OA_ROOM_WRITE_KEY` | `coax/web/room/deploy.sh`, `vm-deploy.sh` |
| `oa-room-ticket-key` | API and shards, until step 5 | `OA_ROOM_TICKET_KEY` | all three |

Two more secrets hold a key between steps and no deploy reads them:
`oa-room-ticket-signing-key-next` and `oa-room-write-master-next`.

The commands below run from the `openagents` checkout with the owner's
`gcloud` login. They pipe each key from `openssl` or Secret Manager straight
into the next command, so no key reaches the terminal, a file or the shell
history. Set the project once:

```sh
P=openagentsgemini
```

## Rollout

Do the steps in order. Each step works with the steps before it, and you can
stop after any step.

### 1. Make the ticket key and give the rooms its public key

```sh
openssl rand -hex 32 | tr -d '\n' | gcloud secrets create oa-room-ticket-signing-key-next --project $P --replication-policy automatic --data-file=-
OA_ROOM_TICKET_SIGNING_KEY="$(gcloud secrets versions access latest --secret oa-room-ticket-signing-key-next --project $P)" \
  coax/web/tool.sh coax/web/room/seat-ticket-profile.coax pubkey | tr -d '\n' |
  gcloud secrets create oa-room-ticket-pubkeys --project $P --replication-policy automatic --data-file=-
```

Deploy the rooms (`coax/web/room/vm-deploy.sh secret`, then the usual room
deploy). Each shard prints `seat tickets checked (v2-v4, v5)`. Rooms still
take the API's current tickets.

### 2. Make the write master and deploy the API

```sh
openssl rand -hex 32 | tr -d '\n' | gcloud secrets create oa-room-write-master --project $P --replication-policy automatic --data-file=-
```

Deploy the API (`coax/web/api/deploy.sh`). It now checks a shard's own key
when a write names one, and still takes writes signed with the shared key.
Its migration 0156 runs with the deploy: each new seat ticket now notes the
shard it seats, and an item or checkpoint write signed with another shard's own
key for that player is refused (403). Nothing is refused yet, because no shard
signs with its own key until step 3.

### 3. Give each shard its write key and deploy the rooms

For each production shard K (1 to 6):

```sh
K=1
OA_ROOM_WRITE_MASTER="$(gcloud secrets versions access latest --secret oa-room-write-master --project $P)" \
  coax/web/tool.sh coax/web/room/room-write-tool.coax key $K | tr -d '\n' |
  gcloud secrets create oa-room-write-key-s$K --project $P --replication-policy automatic --data-file=-
```

Deploy the rooms (`vm-deploy.sh secret`, then the room deploy). Each shard
prints `writes signed as shard K with its own key`. Check the admin room view:
every shard's stats and directory rows keep updating.

Do step 2 before step 3. An API without the master refuses a shard's own
signature.

### 4. Make the API sign version 5 tickets

```sh
gcloud secrets versions access latest --secret oa-room-ticket-signing-key-next --project $P |
  gcloud secrets create oa-room-ticket-signing-key --project $P --replication-policy automatic --data-file=-
gcloud secrets delete oa-room-ticket-signing-key-next --project $P
```

Deploy the API. Join Everglade: the page's ticket starts with `v5.`. The room
deploy's checks now sign v5 tickets too.

Do step 1 before step 4. A room without the public key refuses v5 tickets.

### 5. Turn the shared key off

After every shard and the API run steps 1 to 4, wait 11 minutes so that
every v2, v3 and v4 ticket has expired (a ticket lives 10 minutes), then:

```sh
gcloud secrets delete oa-room-ticket-key --project $P
```

Deploy the rooms, then the API. The shards print no `v2-v4`, and the API now
refuses a write without a shard's signature. Nothing a room holds can sign a
ticket or write as another shard.

To undo step 5 before you delete the secret, skip it. After the delete, undo
it by creating a new `oa-room-ticket-key` (`openssl rand -hex 32`) and
deploying the API and the rooms.

## Rotating the ticket key

1. Make the next key as in step 1, but write the public key as a second
   version of `oa-room-ticket-pubkeys` holding both keys, current first:

   ```sh
   openssl rand -hex 32 | tr -d '\n' | gcloud secrets create oa-room-ticket-signing-key-next --project $P --replication-policy automatic --data-file=-
   { gcloud secrets versions access latest --secret oa-room-ticket-pubkeys --project $P; printf ,
     OA_ROOM_TICKET_SIGNING_KEY="$(gcloud secrets versions access latest --secret oa-room-ticket-signing-key-next --project $P)" \
       coax/web/tool.sh coax/web/room/seat-ticket-profile.coax pubkey | tr -d '\n'; } |
     gcloud secrets versions add oa-room-ticket-pubkeys --project $P --data-file=-
   ```

2. Deploy the rooms. They take tickets under either key.
3. Move the next key into place and deploy the API:

   ```sh
   gcloud secrets versions access latest --secret oa-room-ticket-signing-key-next --project $P |
     gcloud secrets versions add oa-room-ticket-signing-key --project $P --data-file=-
   gcloud secrets delete oa-room-ticket-signing-key-next --project $P
   ```

4. After 11 minutes (the longest a ticket lives, plus clock skew), write the
   new public key alone as the next version of `oa-room-ticket-pubkeys` and
   deploy the rooms:

   ```sh
   gcloud secrets versions access latest --secret oa-room-ticket-pubkeys --project $P | cut -d, -f2 | tr -d '\n' |
     gcloud secrets versions add oa-room-ticket-pubkeys --project $P --data-file=-
   ```

Each ticket names its key's ID (the first 8 hex digits of the public key), so
rooms with two keys check each ticket with the right one.

## Rotating the write master

1. Make the next master, and give the API both masters, current first:

   ```sh
   openssl rand -hex 32 | tr -d '\n' | gcloud secrets create oa-room-write-master-next --project $P --replication-policy automatic --data-file=-
   { gcloud secrets versions access latest --secret oa-room-write-master --project $P; printf ,
     gcloud secrets versions access latest --secret oa-room-write-master-next --project $P; } |
     gcloud secrets versions add oa-room-write-master --project $P --data-file=-
   ```

2. Deploy the API. It takes writes under either master.
3. For each shard K, add its key under the next master as a new version, then
   deploy the rooms:

   ```sh
   OA_ROOM_WRITE_MASTER="$(gcloud secrets versions access latest --secret oa-room-write-master-next --project $P)" \
     coax/web/tool.sh coax/web/room/room-write-tool.coax key $K | tr -d '\n' |
     gcloud secrets versions add oa-room-write-key-s$K --project $P --data-file=-
   ```

4. Give the API the next master alone, deploy it, and delete the next secret:

   ```sh
   gcloud secrets versions access latest --secret oa-room-write-master-next --project $P |
     gcloud secrets versions add oa-room-write-master --project $P --data-file=-
   gcloud secrets delete oa-room-write-master-next --project $P
   ```

A shard's key text starts with its master's ID, and the shard sends that ID in
`X-Room-Signer`, so an API with two masters checks each write with the right
one.

## A leaked key

- **One shard's write key.** It can write only as that shard. Rotate the write
  master. The API refuses that shard's old key once step 4 of the rotation
  is done.
- **The ticket signing key.** Rotate the ticket key, but go straight to the
  new public key alone in step 1 instead of adding it, so the old key's
  tickets stop seating at once.
- **A room host.** It holds the public keys, which are not secret, and its
  shard's write key. Rotate the write master.

## Checks

```sh
coax/bin/coax test aethyr/session/seat-ticket.coax
coax/web/tool.sh coax/web/room/seat-ticket-profile.coax proto/verse/seat-ticket-v5.json
coax/bin/coax test coax/web/room/room-write.coax
coax/web/tool.sh coax/web/room/room-write-tool.coax profile proto/verse/room-write-v1.json
coax/web/api/test.sh   # tests/room_checks.coax rc_write_keys: a shard writes as itself only
```
