# Owner actions

## R0 adoption gate

The benchmark/source-debugging implementation is usable. Closing R0 still
needs a representative physical iPhone run, agreement on device median/frame
tail budgets, and an independent review of the frozen native-reference f32
reader/full-body-mask adaptation and tolerance. The updated Mac pose comparison
passes the unchanged proposed 5% median gate in all three profiles, with raw
tails/outliers retained. That is measured median evidence, not agreement on
device/frame-tail budgets or an independent adoption sign-off.

Independent invariant/oracle review and its enforcement on owned
infrastructure remain separate roadmap work. This entry records those future
owner/reviewer decisions; it does not block building or using this slice.

## Coax Nostr queue: site releases from the Linux worker

`claude-nostr` signs queue events from the Linux host, but it cannot run
`coax/web/release.sh` there. The page build uses the Mac's `/opt/homebrew`
Wasm toolchain, and the release goldens are Mac renders. The relay is current;
only the page lags. Either run `coax/web/release.sh` from the Mac now and
then, or decide whether the Linux host may release. That would need Linux
goldens or a policy for them, and `COAXIAL_*` set to
`~/.cache/coax-wasm-toolchain`.
