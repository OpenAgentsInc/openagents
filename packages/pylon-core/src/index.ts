/**
 * @openagentsinc/pylon-core
 *
 * Typed Effect engine services extracted out of `apps/pylon/src` per the
 * accepted "fold Pylon into Khala Code" proposal
 * (`docs/fable/2026-07-08-pylon-into-khala-code-proposal.md`, §3/§5) and
 * tracked in GitHub issue #8578 (PY-1).
 *
 * The engine decomposes into four service boundaries:
 *   - custody  (P1): per-account Codex/Claude homes, registry, quota, health
 *   - executor (P2): local coding-delegation runs, dispatch, closeouts
 *   - presence (P3): go-online / heartbeat / counted capacity refs
 *   - wallet   (P5): the Spark rail — a live, preserved payment rail that
 *                    lives behind its own service boundary and never inside a
 *                    GUI process
 *
 * Extraction is incremental: modules move here bottom-up (leaf dependencies
 * first) and the original `apps/pylon/src` files become thin re-export shims
 * so existing CLI/daemon/desktop consumers keep compiling unchanged.
 *
 * Service modules are re-exported from this entrypoint as they land. The
 * scaffold intentionally starts empty (issue #8578 step 1); custody lands in
 * step 2.
 */

export * as Shared from "./shared/index.js"
export * as Custody from "./custody/index.js"
