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
const envDefaultModel = process.env.OPENCLAW_DEFAULT_MODEL?.trim();
const openRouterKey = process.env.OPENROUTER_API_KEY?.trim();
const openAiKey = process.env.OPENAI_API_KEY?.trim();
const anthropicKey = process.env.ANTHROPIC_API_KEY?.trim();

const defaultModel = envDefaultModel
  || (openRouterKey ? 'openai/gpt-4o-mini' : '')
  || (openAiKey ? 'openai/gpt-4o-mini' : '')
  || (anthropicKey ? 'anthropic/claude-sonnet-4.5' : '');

let config = {};
try {
  config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
} catch {
  config = {};
}

// Remove legacy "agent" config block; newer gateway versions reject it.
if (Object.prototype.hasOwnProperty.call(config, 'agent')) {
  if (config.agent && typeof config.agent === 'object') {
    config.agents = config.agents || {};
    config.agents.defaults = config.agents.defaults || {};
    for (const [key, value] of Object.entries(config.agent)) {
      if (config.agents.defaults[key] === undefined) {
        config.agents.defaults[key] = value;
      }
    }
  }
  delete config.agent;
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
if (defaultModel) {
  const ensureModelDefaults = (defaults) => {
    if (!defaults || typeof defaults !== 'object') return;
    if (!defaults.model) {
      defaults.model = { primary: defaultModel };
    } else if (typeof defaults.model === 'string') {
      defaults.model = { primary: defaults.model };
    } else if (typeof defaults.model === 'object') {
      defaults.model.primary = defaults.model.primary || defaultModel;
    }
    if (!defaults.models || typeof defaults.models !== 'object') {
      defaults.models = {};
    }
    if (defaults.models && typeof defaults.models === 'object') {
      defaults.models[defaultModel] = defaults.models[defaultModel] || {};
    }
  };

  config.agents = config.agents || {};
  config.agents.defaults = config.agents.defaults || {};
  ensureModelDefaults(config.agents.defaults);
}
fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
EOFNODE

# Normalize any legacy config keys (e.g. agent -> agents.defaults) before gateway boot.
clawdbot doctor --fix >/dev/null 2>&1 || true

if [ -n "$OPENCLAW_GATEWAY_TOKEN" ]; then
  exec openclaw gateway --port 18789 --allow-unconfigured --bind "$BIND_MODE" --token "$OPENCLAW_GATEWAY_TOKEN"
else
  exec openclaw gateway --port 18789 --allow-unconfigured --bind "$BIND_MODE"
fi
