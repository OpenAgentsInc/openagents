#!/bin/bash
set -e

if pgrep -f "openclaw gateway" > /dev/null 2>&1 || pgrep -f "clawdbot gateway" > /dev/null 2>&1; then
    echo "Gateway already running, exiting."
    exit 0
fi

CONFIG_DIR="/root/.clawdbot"
CONFIG_FILE="$CONFIG_DIR/clawdbot.json"
WORKSPACE_DIR="/root/clawd"
SKILLS_DIR="$WORKSPACE_DIR/skills"
BIND_MODE="${OPENCLAW_BIND_MODE:-loopback}"

mkdir -p "$CONFIG_DIR" "$WORKSPACE_DIR" "$SKILLS_DIR" "/data/openclaw"

if [ ! -f "$CONFIG_FILE" ]; then
  cat > "$CONFIG_FILE" << 'EOFCONFIG'
{
  "agents": {
    "defaults": {
      "workspace": "/root/clawd"
    }
  },
  "gateway": {
    "port": 18789,
    "mode": "local"
  }
}
EOFCONFIG
fi

node << 'EOFNODE'
const fs = require('fs');
const configPath = '/root/.clawdbot/clawdbot.json';
const token = process.env.OPENCLAW_GATEWAY_TOKEN;
let config = {};
try {
  config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
} catch {
  config = {};
}
config.gateway = config.gateway || {};
config.gateway.http = config.gateway.http || {};
config.gateway.http.endpoints = config.gateway.http.endpoints || {};
config.gateway.http.endpoints.responses = config.gateway.http.endpoints.responses || {};
config.gateway.http.endpoints.responses.enabled = true;
if (token) {
  config.gateway.auth = config.gateway.auth || {};
  config.gateway.auth.token = token;
}
fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
EOFNODE

if [ -n "$OPENCLAW_GATEWAY_TOKEN" ]; then
  exec openclaw gateway --port 18789 --allow-unconfigured --bind "$BIND_MODE" --token "$OPENCLAW_GATEWAY_TOKEN"
else
  exec openclaw gateway --port 18789 --allow-unconfigured --bind "$BIND_MODE"
fi
