import { html } from "@openagentsinc/psionic"

interface HeaderOptions {
  current?: string
}

export function sharedHeader({ current }: HeaderOptions = {}) {
  return html`
    <!-- ASCII Box Header -->
    <header class="ascii-header" box-="square" shear-="bottom">
      <div class="header-content">
        <a href="/" class="brand">OpenAgents</a>
        <nav class="header-nav">
          <!-- <a href="/chat" class="nav-link ${current === "chat" ? "active" : ""}">◊ Chat</a> -->
          <a href="/channels" class="nav-link ${current === "channels" ? "active" : ""}">▬ Channels</a>
          <!-- <a href="/agents" class="nav-link ${current === "agents" ? "active" : ""}">◆ Agents</a> -->
          <a href="/docs" class="nav-link ${current === "docs" ? "active" : ""}">§ Docs</a>
          <a href="/blog" class="nav-link ${current === "blog" ? "active" : ""}">¶ Blog</a>
          <a href="/admin" class="nav-link admin-link ${
    current === "admin" ? "active" : ""
  }" style="display: none;">⚙ Admin</a>
          <div class="theme-switcher-container">
            <select id="theme-select" class="theme-select" onchange="switchTheme(this.value)">
              <option value="zinc">Zinc</option>
              <option value="ayu">Ayu</option>
              <option value="catppuccin">Catppuccin</option>
              <option value="flexoki">Flexoki</option>
              <option value="gruvbox">Gruvbox</option>
              <option value="monokai">Monokai</option>
              <option value="nord">Nord</option>
              <option value="onedark">One Dark</option>
              <option value="tokyonight">Tokyo Night</option>
              <option value="tron">Tron</option>
            </select>
          </div>
        </nav>
      </div>
    </header>

    <style>
      /* ASCII Box Header */
      .ascii-header {
        flex-shrink: 0;
        background: var(--background0);
        padding: 1rem 2rem;
        --box-border-color: color-mix(in srgb, var(--background3) 50%, transparent);
        --box-border-width: 1px;
      }

      /* Override box padding for more horizontal spacing */
      .ascii-header[box-] {
        padding-left: 2rem;
        padding-right: 2rem;
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
        text-decoration: none;
        transition: color 0.2s ease;
      }

      .brand:hover {
        color: var(--foreground1);
      }

      .header-nav {
        display: flex;
        align-items: center;
        gap: 1.5rem;
      }

      .nav-link {
        color: var(--foreground1);
        text-decoration: none;
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        transition: all 0.2s ease;
        font-size: 0.9rem;
      }

      .nav-link:hover,
      .nav-link.active {
        color: var(--foreground0);
        background: var(--background1);
      }

      .theme-switcher-container {
        display: flex;
        align-items: center;
        margin-left: 1rem;
      }

      .theme-select {
        background: var(--background1);
        color: var(--foreground1);
        border: 1px solid var(--background3);
        padding: 0.5rem 1.25rem 0.5rem 0.75rem;
        font-family: inherit;
        font-size: 0.85rem;
        cursor: pointer;
        transition: all 0.2s ease;
        min-width: 120px;
      }

      .theme-select:focus {
        outline: none;
        border-color: var(--foreground1);
        background: var(--background2);
      }

      .theme-select:hover {
        border-color: var(--background3);
        background: var(--background2);
      }

      .theme-select option {
        background: var(--background1);
        color: var(--foreground1);
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

        .header-nav {
          gap: 1rem;
        }

        .theme-switcher-container {
          margin-left: 0;
        }
      }
    </style>

    <script>
      // Theme switching functionality
      function switchTheme(theme) {
        // Remove existing theme classes
        document.body.classList.remove('theme-zinc', 'theme-ayu', 'theme-catppuccin', 'theme-flexoki', 'theme-gruvbox', 'theme-monokai', 'theme-nord', 'theme-onedark', 'theme-tokyonight', 'theme-tron');
        
        // Add new theme class
        document.body.classList.add('theme-' + theme);
        
        // Save theme preference
        localStorage.setItem('openagents-theme', theme);
      }
      
      // Initialize theme on page load
      (function() {
        const savedTheme = localStorage.getItem('openagents-theme') || 'zinc';
        const themeSelect = document.getElementById('theme-select');
        
        if (themeSelect) {
          // Handle old 'light' or 'zinc-light' values for backwards compatibility
          if (savedTheme === 'light' || savedTheme === 'zinc-light') {
            themeSelect.value = 'zinc';
            switchTheme('zinc');
          } else {
            themeSelect.value = savedTheme;
            switchTheme(savedTheme);
          }
        }
      })();
      
      // Show admin link only on localhost
      (function() {
        const isLocalhost = ['localhost', '127.0.0.1', '::1'].includes(window.location.hostname);
        const adminLink = document.querySelector('.admin-link');
        
        if (adminLink && isLocalhost) {
          adminLink.style.display = 'inline-block';
        }
      })();
      
      // Make switchTheme available globally
      window.switchTheme = switchTheme;
    </script>
  `
}
