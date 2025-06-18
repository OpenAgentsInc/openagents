import { document, html } from "@openagentsinc/psionic"
import { themeSwitcher } from "../components/theme-switcher"
import { baseStyles } from "../styles"

export function home() {
  return document({
    title: "OpenAgents",
    styles: baseStyles,
    body: html`
      <!-- ASCII Box Header -->
      <header class="ascii-header" box-="square" shear-="bottom">
        <div class="header-content">
          <span class="brand">OpenAgents</span>
          <div class="theme-switcher-container">
            ${themeSwitcher()}
          </div>
        </div>
      </header>

      <!-- Main Content -->
      <main class="homepage-main">
        <div class="centered-card" box-="square">
          <h1>OpenAgents</h1>
        </div>
      </main>

      <style>
        body {
          background: var(--background0);
          margin: 0;
          padding: 0;
          min-height: 100vh;
          font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
        }

        /* ASCII Box Header */
        .ascii-header {
          position: sticky;
          top: 0;
          z-index: 100;
          background: var(--background0);
          padding: 1rem 2rem;
        }

        .header-content {
          display: flex;
          justify-content: space-between;
          align-items: center;
          max-width: 1200px;
          margin: 0 auto;
        }

        .brand {
          font-size: 1.2rem;
          font-weight: 700;
          color: var(--foreground0);
        }

        .theme-switcher-container {
          display: flex;
          align-items: center;
        }

        .theme-switcher select {
          background: var(--background1);
          color: var(--foreground1);
          border: 1px solid var(--foreground2);
          border-radius: 4px;
          padding: 0.5rem;
          font-family: inherit;
          font-size: 0.9rem;
        }

        .theme-switcher select:focus {
          outline: none;
          border-color: var(--foreground0);
        }

        /* Main Content */
        .homepage-main {
          display: flex;
          align-items: center;
          justify-content: center;
          min-height: calc(100vh - 120px);
          padding: 2rem;
        }

        .centered-card {
          text-align: center;
          padding: 3rem 4rem;
          background: var(--background1);
          min-width: 300px;
        }

        .centered-card h1 {
          margin: 0;
          font-size: 2.5rem;
          font-weight: 700;
          color: var(--foreground0);
        }

        /* Responsive */
        @media (max-width: 768px) {
          .ascii-header {
            padding: 1rem;
          }

          .header-content {
            flex-direction: column;
            gap: 1rem;
          }

          .centered-card {
            padding: 2rem;
            min-width: 250px;
          }

          .centered-card h1 {
            font-size: 2rem;
          }
        }
      </style>
    `
  })
}