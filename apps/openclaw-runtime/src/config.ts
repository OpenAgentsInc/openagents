export const GATEWAY_PORT = 18789;
export const GATEWAY_WS_URL = `ws://localhost:${GATEWAY_PORT}`;
export const GATEWAY_HTTP_URL = `http://127.0.0.1:${GATEWAY_PORT}`;

export const CONFIG_DIR = '/root/.clawdbot';
export const CONFIG_FILE = `${CONFIG_DIR}/clawdbot.json`;
export const WORKSPACE_DIR = '/root/clawd';
export const SKILLS_DIR = `${WORKSPACE_DIR}/skills`;

export const BACKUP_DIR = '/data/openclaw';
export const BACKUP_ARCHIVE = `${BACKUP_DIR}/backup.tar.gz`;

export const CLI_TIMEOUT_MS = 20_000;
export const GATEWAY_HTTP_TIMEOUT_MS = 45_000;
export const GATEWAY_STREAM_TIMEOUT_MS = 120_000;
export const STARTUP_TIMEOUT_MS = 90_000;
