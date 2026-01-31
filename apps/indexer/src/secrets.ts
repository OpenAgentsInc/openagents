/**
 * Lightweight secret scanning before writing to D1 or public UI.
 * If matched: store raw in quarantine (or skip), store redacted in D1 (contains_secrets=true).
 */

import type { SecretScanResult } from './types';

const PATTERNS: RegExp[] = [
  /\bmoltbook_[a-zA-Z0-9_-]{20,}\b/gi,
  /\b(?:sk|pk)_[a-zA-Z0-9]{20,}\b/gi,
  /-----BEGIN (?:RSA |EC |DSA )?PRIVATE KEY-----/i,
  /-----BEGIN OPENSSH PRIVATE KEY-----/i,
  /\b(?:AKIA|A3T|AGPA|AIDA|AROA|AIPA|ANPA|ANVA|ASIA)[A-Z0-9]{16}\b/gi,
  /\bghp_[a-zA-Z0-9]{36}\b/gi,
  /\bgho_[a-zA-Z0-9]{36}\b/gi,
];

const REDACT = '[REDACTED_SECRET]';

export function scanSecrets(text: string | null | undefined): SecretScanResult {
  if (!text || typeof text !== 'string') {
    return { hasSecrets: false };
  }
  let redacted = text;
  let hasSecrets = false;
  for (const re of PATTERNS) {
    if (re.test(text)) {
      hasSecrets = true;
      redacted = redacted.replace(re, REDACT);
    }
  }
  return hasSecrets ? { hasSecrets: true, redactedContent: redacted } : { hasSecrets: false };
}

export function redactForD1(content: string | null | undefined, scan: SecretScanResult): string {
  if (!content) return '';
  if (scan.hasSecrets && scan.redactedContent) return scan.redactedContent;
  return content;
}
