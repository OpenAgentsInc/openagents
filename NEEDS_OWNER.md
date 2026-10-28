# Owner actions

## R0 adoption gate

The benchmark/source-debugging implementation is usable. Closing R0 still
needs a representative physical iPhone run, agreement on device median/frame
tail budgets, and an independent review of the frozen native-reference f32
reader/full-body-mask adaptation and tolerance. The current Mac optimized
pose comparison misses the proposed 5% regression budget; do not sign off
performance based on the passing correctness or Simulator tests.

Independent invariant/oracle review and its enforcement on owned
infrastructure remain separate roadmap work. This entry records those future
owner/reviewer decisions; it does not block building or using this slice.
