# Bundled media path projection

`hmb_path(url, out)` returns the complete byte length or -1 without changing
`out`. It uses the existing checked HTTP target normalization, drops the
query and fragment for file lookup, maps exact `/worlds` and `/worlds/…`
to the bundled app's `/realms` tree, and maps `/` to `/index.html`.
The original URL remains the immutable image-cache and 607 identity.

Percent escapes remain encoded in the filesystem lookup, matching the
repository's `coax/web/serve.c` static-file mapper. This API does not perform
a second percent decode, select a network origin or search a private build
directory. Missing and unsupported bundled files remain failed/not-ready
image observations through the existing cache/decoder contract.

The SDK appends the returned path to its actual bundle mount, resolves its
physical canonical path, and passes both actual canonical strings to
`hmb_inside(root, path)`. That predicate requires an absolute canonical root
and an exact child separator. Only then may the SDK observe/open a regular
file. The SDK still owns symlink resolution, file type/length observations,
exact bounded reads and physical race/error handling.

The module is in the existing native renderer facade, rather than the
separate texture/decoder object, so HTTP normalization exports have one
physical owner. It does not enable a host/default change. Its final root
mutation gate kills all 65 sites with no survivors or equivalents; the
initial redundant query scan and its surviving mutation remain retained.
