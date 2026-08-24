# dir_stats

The proof plugin for the host's second capability import,
`openagents.list_dir`: one bounded directory listing through a declared
read-only mount, named by mount index and mount-relative path. It reads no
file contents and reaches nothing outside the mount; the escape tests in
`packages/openagents-cli/test/coder-plugin-list-dir.test.ts` drive this
module through the real boundary the way
`coder-plugin-mounts.test.ts` drives `file_stats` through `read_file`.
