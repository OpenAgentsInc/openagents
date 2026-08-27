# dir_stats

The proof plugin for the host's second capability import,
`openagents.list_dir`: one bounded directory listing through a declared
read-only mount, named by mount index and mount-relative path. It reads no
file contents and reaches nothing outside the mount; the escape tests in
`crates/openagents-cli/tests/plugin_host_test.rs` drive this module through
the real boundary.
