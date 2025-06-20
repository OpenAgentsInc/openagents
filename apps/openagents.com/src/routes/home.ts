import { document, html } from "@openagentsinc/psionic"
import { Agent } from "@openagentsinc/sdk"
import { sharedHeader } from "../components/shared-header"
import { baseStyles } from "../styles"

export async function home() {
  // Generate a new agent using the SDK
  const mnemonic = await Agent.generateMnemonic()
  const agent = await Agent.createFromMnemonic(mnemonic, {
    name: "Demo Agent"
  })

  // Log the agent's public key
  console.log("Generated agent npub:", agent.nostrKeys.public)

  return document({
    title: "OpenAgents",
    styles: baseStyles,
    body: html`
      <!-- Fixed Layout Container -->
      <div class="fixed-layout">
        ${sharedHeader({ current: "home" })}

        <!-- Main Content -->
        <main class="homepage-main">
          <div class="launch-notice" box-="square">
            <div class="notice-title">Welcome to OpenAgents</div>
            <div class="notice-content">
              <p>We're launching a few new products on <strong>Saturday June 21</strong>.</p>
              <p>In the meantime, explore our resources:</p>
              <div class="notice-links">
                <a href="/blog" is-="button" variant-="foreground1" box-="square">
                  Read our blog →
                </a>
                <a href="/docs" is-="button" variant-="foreground1" box-="square">
                  View the docs →
                </a>
              </div>
            </div>
          </div>
        </main>
      </div>

      <style>
        html, body {
          background: var(--background0);
          margin: 0;
          padding: 0;
          height: 100vh;
          overflow: hidden;
          font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
          position: fixed;
          width: 100%;
        }

        /* Fixed Header for Homepage */
        .ascii-header {
          position: fixed !important;
          top: 0;
          left: 0;
          right: 0;
          z-index: 1000;
        }

        /* Fixed Layout */
        .fixed-layout {
          width: 100vw;
          height: 100vh;
          display: flex;
          flex-direction: column;
          overflow: hidden;
          padding-top: 80px; /* Account for fixed header height */
        }

        /* Main Content */
        .homepage-main {
          flex: 1;
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 2rem;
          overflow: hidden;
        }

        .launch-notice {
          text-align: center;
          padding: 3rem 4rem;
          background: var(--background1);
          min-width: 400px;
          max-width: 600px;
        }

        .notice-title {
          margin: 0 0 2rem 0;
          font-size: 2rem;
          font-weight: 700;
          color: var(--foreground0);
        }

        .notice-content {
          color: var(--foreground1);
        }

        .notice-content p {
          margin: 1rem 0;
          line-height: 1.6;
        }

        .notice-content strong {
          color: var(--foreground0);
          font-weight: 600;
        }

        .notice-links {
          display: flex;
          gap: 1.5rem;
          justify-content: center;
          margin-top: 2.5rem;
        }

        .notice-links a {
          text-decoration: none;
        }

        /* Responsive */
        @media (max-width: 768px) {
          .launch-notice {
            padding: 2rem;
            min-width: 250px;
          }

          .notice-title {
            font-size: 1.5rem;
          }

          .notice-links {
            flex-direction: column;
            gap: 1rem;
          }
        }
      </style>
    `
  })
}
