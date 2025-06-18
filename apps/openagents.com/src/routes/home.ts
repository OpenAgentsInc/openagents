import { document, html } from "@openagentsinc/psionic"
import { sharedHeader } from "../components/shared-header"
import { baseStyles } from "../styles"

export function home() {
  return document({
    title: "OpenAgents",
    styles: baseStyles,
    body: html`
      <!-- Fixed Layout Container -->
      <div class="fixed-layout">
        ${sharedHeader({ current: "home" })}

        <!-- Main Content -->
        <main class="homepage-main">
          <div class="centered-card" box-="square">
            <div class="card-title">OpenAgents</div>
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

        .centered-card {
          text-align: center;
          padding: 3rem 4rem;
          background: var(--background1);
          min-width: 300px;
        }

        .card-title {
          margin: 0;
          font-size: 2.5rem;
          font-weight: 700;
          color: var(--foreground0);
        }

        /* Responsive */
        @media (max-width: 768px) {
          .centered-card {
            padding: 2rem;
            min-width: 250px;
          }

          .card-title {
            font-size: 2rem;
          }
        }
      </style>
    `
  })
}
