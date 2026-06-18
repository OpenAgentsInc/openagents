/**
 * `@openagentsinc/replay-clips` — replay clip job, manifest, and camera-path
 * DSL contracts for the replay clips production service (EPIC #5411).
 *
 * Schema-only package. It defines the public-safe contracts an agent or
 * operator uses to request a directed replay clip (#5430), the camera-path DSL
 * an agent emits to direct the camera (#5433), and the public-safe output
 * manifest the render box produces. It grants no settlement, payout,
 * deployment, accepted-work, provider, wallet, or public-claim authority.
 *
 * Rendering itself runs on owned local/CI/Container infrastructure (#5431),
 * never inside the Cloudflare Worker. The Worker may host job records and serve
 * finished refs only (#5432).
 */
export * from "./camera-path.js"
export * from "./clip-job.js"
