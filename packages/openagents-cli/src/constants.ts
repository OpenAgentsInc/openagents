/**
 * The base path for all versioned OpenAgents API routes.
 *
 * All versioned REST resources live under this prefix. A passthrough path
 * without a leading slash resolves under it.
 */
export const API_BASE_PATH = "/api/v1/";

/**
 * The versioned API prefix without a trailing slash, used to construct
 * route paths for specific endpoints across CLI clients.
 */
export const API_VERSION_PATH = "/api/v1";

/**
 * The endpoint path for coder threads.
 */
export const THREADS_PATH = `${API_VERSION_PATH}/threads`;

/**
 * The endpoint path for operator fleet targets.
 */
export const FLEET_TARGETS_PATH = `${API_VERSION_PATH}/admin/forge/targets`;

/**
 * The endpoint path for the account's cloud memories.
 *
 * Named once because two callers share it: the Effect-based `MemoryClient`
 * behind `openagents memory`, and the `remember` tool a coder session declares,
 * which posts here directly rather than through the CLI runtime.
 */
export const MEMORIES_PATH = `${API_VERSION_PATH}/memories`;
