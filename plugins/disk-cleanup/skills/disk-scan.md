# Scan disk use

Run a full, read-only scan of this computer's disk use and present what it
found. This skill deletes nothing.

Enable the plugin once (it can only scan), then run the scan with the host
command:

```sh
openagents plugin enable disk-cleanup
openagents --json background scan disk-cleanup
```

The scan reads the rule in this plugin (`disk-cleanup`) and every candidate
class it names. It lists each candidate folder with its size, how long since it
was last used, and why it qualifies. It lists each folder it keeps and why, and
the folders it does not clean because they are not known caches. It writes no
state and removes nothing.

Present the result to the person in this order:

1. The free space on each watched volume.
2. Each class with its folder count and total reclaimable size, largest first.
3. The biggest kept folders, with the reason each one stays.
4. The folders that are not known caches, with their sizes.

Name the reclaimable total and the number of folders. Do not say that anything
was removed.

Do not delete anything from this skill. A later step may remove folders the
person chooses from the list; until that step exists, leave the folders as they
are and say so.
