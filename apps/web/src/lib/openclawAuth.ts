export const AGENT_KEY_HEADER = 'x-oa-agent-key';
const AGENT_AUTH_PREFIX = 'agent ';

export const extractAgentKey = (headers: Headers): string | null => {
  const direct = headers.get(AGENT_KEY_HEADER);
  if (direct && direct.trim()) {
    return direct.trim();
  }
  const auth = headers.get('authorization');
  if (!auth) return null;
  const trimmed = auth.trim();
  if (trimmed.length <= AGENT_AUTH_PREFIX.length) return null;
  if (!trimmed.toLowerCase().startsWith(AGENT_AUTH_PREFIX)) return null;
  const key = trimmed.slice(AGENT_AUTH_PREFIX.length).trim();
  return key ? key : null;
};
