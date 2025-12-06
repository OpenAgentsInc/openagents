/**
 * Test HTML Template Generator
 *
 * Generates complete HTML documents for webview tests.
 * Includes test harness, assertion helpers, and widget mounting code.
 */

export interface TestHTMLOptions {
  /** Widget bundle code (IIFE format) */
  widgetBundle?: string
  /** CSS styles to include */
  styles?: string
  /** Compiled test steps (JavaScript) */
  testSteps: string
  /** WebSocket URL for integration tests */
  wsUrl?: string
  /** Initial state for widget */
  initialState?: unknown
}

/**
 * Generate test harness JavaScript code.
 * This runs IN the webview and provides assertion helpers.
 */
const generateHarnessCode = (wsUrl?: string): string => `
  // Test state
  window.__effuseTestResults = [];
  window.__effuseTestDone = false;

  // Assertion helpers
  const assert = {
    eq: (a, b, msg) => {
      if (a !== b) throw new Error(msg || \`Expected \${JSON.stringify(a)} to equal \${JSON.stringify(b)}\`);
    },

    neq: (a, b, msg) => {
      if (a === b) throw new Error(msg || \`Expected \${JSON.stringify(a)} to not equal \${JSON.stringify(b)}\`);
    },

    contains: (str, sub, msg) => {
      if (typeof str !== 'string') throw new Error(\`Expected string, got \${typeof str}\`);
      if (!str.includes(sub)) throw new Error(msg || \`Expected "\${str}" to contain "\${sub}"\`);
    },

    visible: (sel, msg) => {
      const el = document.querySelector(sel);
      if (!el) throw new Error(msg || \`Element \${sel} not found\`);
      const style = getComputedStyle(el);
      if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {
        throw new Error(msg || \`Element \${sel} is not visible\`);
      }
    },

    hidden: (sel, msg) => {
      const el = document.querySelector(sel);
      if (!el) return; // Not found = hidden
      const style = getComputedStyle(el);
      if (style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0') {
        throw new Error(msg || \`Element \${sel} is visible (expected hidden)\`);
      }
    },

    exists: (sel, msg) => {
      const el = document.querySelector(sel);
      if (!el) throw new Error(msg || \`Element \${sel} not found\`);
    },

    notExists: (sel, msg) => {
      const el = document.querySelector(sel);
      if (el) throw new Error(msg || \`Element \${sel} exists (expected not to exist)\`);
    },

    count: (sel, expected, msg) => {
      const els = document.querySelectorAll(sel);
      if (els.length !== expected) {
        throw new Error(msg || \`Expected \${expected} elements matching \${sel}, found \${els.length}\`);
      }
    },

    text: (sel, expected, msg) => {
      const el = document.querySelector(sel);
      if (!el) throw new Error(\`Element \${sel} not found\`);
      const actual = el.textContent || '';
      if (!actual.includes(expected)) {
        throw new Error(msg || \`Expected "\${sel}" text to contain "\${expected}", got "\${actual}"\`);
      }
    },

    attr: (sel, attrName, expected, msg) => {
      const el = document.querySelector(sel);
      if (!el) throw new Error(\`Element \${sel} not found\`);
      const actual = el.getAttribute(attrName);
      if (actual !== expected) {
        throw new Error(msg || \`Expected \${sel}[\${attrName}] to be "\${expected}", got "\${actual}"\`);
      }
    },

    truthy: (val, msg) => {
      if (!val) throw new Error(msg || \`Expected truthy value, got \${JSON.stringify(val)}\`);
    },

    falsy: (val, msg) => {
      if (val) throw new Error(msg || \`Expected falsy value, got \${JSON.stringify(val)}\`);
    },
  };

  // DOM helpers
  const $ = (sel) => document.querySelector(sel);
  const $$ = (sel) => [...document.querySelectorAll(sel)];
  const $text = (sel) => $(sel)?.textContent || '';
  const $html = (sel) => $(sel)?.innerHTML || '';

  // Async helpers
  const sleep = (ms) => new Promise(r => setTimeout(r, ms));

  const waitFor = async (sel, timeout = 5000) => {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      const el = $(sel);
      if (el) return el;
      await sleep(50);
    }
    throw new Error(\`Timeout waiting for \${sel}\`);
  };

  const waitForText = async (sel, text, timeout = 5000) => {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      const el = $(sel);
      if (el && el.textContent?.includes(text)) return;
      await sleep(50);
    }
    throw new Error(\`Timeout waiting for "\${text}" in \${sel}\`);
  };

  const waitForHidden = async (sel, timeout = 5000) => {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      const el = $(sel);
      if (!el) return;
      const style = getComputedStyle(el);
      if (style.display === 'none' || style.visibility === 'hidden') return;
      await sleep(50);
    }
    throw new Error(\`Timeout waiting for \${sel} to be hidden\`);
  };

  // WebSocket helpers (for integration tests)
  ${wsUrl ? `
  let __ws = null;
  const __messageQueue = [];

  const connectWS = () => new Promise((resolve, reject) => {
    __ws = new WebSocket('${wsUrl}');
    __ws.onopen = () => resolve(__ws);
    __ws.onerror = (e) => reject(new Error('WebSocket connection failed'));
    __ws.onmessage = (e) => {
      try {
        const msg = JSON.parse(e.data);
        __messageQueue.push(msg);
        // Dispatch custom event for listeners
        window.dispatchEvent(new CustomEvent('hud-message', { detail: msg }));
      } catch {}
    };
  });

  const injectMessage = async (msg) => {
    // For integration tests, inject via HTTP API
    const response = await fetch('${wsUrl.replace('/ws', '/inject')}', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(msg),
    });
    if (!response.ok) throw new Error('Failed to inject message');
    // Wait a tick for message to propagate
    await sleep(10);
  };

  const waitForMessage = async (type, timeout = 5000) => {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      const msg = __messageQueue.find(m => m.type === type);
      if (msg) return msg;
      await sleep(50);
    }
    throw new Error(\`Timeout waiting for message type: \${type}\`);
  };
  ` : `
  const injectMessage = async (msg) => {
    // For unit tests, dispatch directly as custom event
    window.dispatchEvent(new CustomEvent('hud-message', { detail: msg }));
    await sleep(10);
  };
  `}

  // Action helpers
  const click = (sel) => {
    const el = $(sel);
    if (!el) throw new Error(\`Cannot click: \${sel} not found\`);
    el.click();
  };

  const type = (sel, text) => {
    const el = $(sel);
    if (!el) throw new Error(\`Cannot type: \${sel} not found\`);
    el.value = text;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  };

  const clear = (sel) => {
    const el = $(sel);
    if (!el) throw new Error(\`Cannot clear: \${sel} not found\`);
    el.value = '';
    el.dispatchEvent(new Event('input', { bubbles: true }));
  };

  const check = (sel, checked = true) => {
    const el = $(sel);
    if (!el) throw new Error(\`Cannot check: \${sel} not found\`);
    el.checked = checked;
    el.dispatchEvent(new Event('change', { bubbles: true }));
  };
`

/**
 * Generate complete test HTML document.
 */
export const generateTestHTML = (options: TestHTMLOptions): string => {
  const { widgetBundle, styles, testSteps, wsUrl, initialState } = options

  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Effuse Test</title>
  <style>
    /* Reset */
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: system-ui, sans-serif; }
    ${styles || ""}
  </style>
</head>
<body>
  <div id="effuse-test-root"></div>

  ${widgetBundle ? `<script>${widgetBundle}</script>` : ""}

  <script>
    ${generateHarnessCode(wsUrl)}

    // Initial state (if provided)
    ${initialState ? `window.__effuseInitialState = ${JSON.stringify(initialState)};` : ""}

    // Run tests
    async function __runEffuseTests() {
      try {
        ${wsUrl ? "await connectWS();" : ""}

        // === TEST STEPS START ===
        ${testSteps}
        // === TEST STEPS END ===

        window.__effuseTestResults.push({ pass: true });
      } catch (e) {
        window.__effuseTestResults.push({
          pass: false,
          error: e.message,
          stack: e.stack,
        });
      }

      window.__effuseTestDone = true;

      // Report results to Bun
      if (window.reportResults) {
        window.reportResults(JSON.stringify(window.__effuseTestResults));
      }
    }

    // Run after DOM ready
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', __runEffuseTests);
    } else {
      __runEffuseTests();
    }
  </script>
</body>
</html>`
}

/**
 * Generate a simple test HTML for a single assertion.
 */
export const generateSimpleTest = (testCode: string): string =>
  generateTestHTML({ testSteps: testCode })
