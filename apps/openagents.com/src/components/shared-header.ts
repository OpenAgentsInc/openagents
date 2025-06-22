import { html } from "@openagentsinc/psionic"

interface HeaderOptions {
  current?: string
}

export function sharedHeader({ current }: HeaderOptions = {}) {
  return html`
    <!-- ASCII Box Header -->
    <header class="fixed top-0 left-0 right-0 z-[1000] box-terminal bg-[--color-terminal-bg] px-8 py-4">
      <div class="flex justify-between items-center max-w-[1200px] mx-auto">
        <a href="/" class="text-xl font-bold text-[--color-terminal-fg] hover:text-[--color-terminal-accent] transition-colors">OpenAgents</a>
        <nav class="flex items-center gap-6">
          <!-- <a href="/chat" class="nav-link ${current === "chat" ? "active" : ""}">◊ Chat</a> -->
          <a href="/channels" class="channels-link ${
    current === "channels" ? "bg-[--color-terminal-border]" : ""
  } text-[--color-terminal-fg] opacity-80 hover:opacity-100 hover:bg-[--color-terminal-border] px-2 py-1 rounded transition-all text-sm hidden">▬ Channels</a>
          <!-- <a href="/agents" class="nav-link ${current === "agents" ? "active" : ""}">◆ Agents</a> -->
          <a href="/docs" class="${
    current === "docs" ? "bg-[--color-terminal-border]" : ""
  } text-[--color-terminal-fg] opacity-80 hover:opacity-100 hover:bg-[--color-terminal-border] px-2 py-1 rounded transition-all text-sm">§ Docs</a>
          <a href="/blog" class="${
    current === "blog" ? "bg-[--color-terminal-border]" : ""
  } text-[--color-terminal-fg] opacity-80 hover:opacity-100 hover:bg-[--color-terminal-border] px-2 py-1 rounded transition-all text-sm">¶ Blog</a>
          <a href="/admin" class="admin-link ${
    current === "admin" ? "bg-[--color-terminal-border]" : ""
  } text-[--color-terminal-fg] opacity-80 hover:opacity-100 hover:bg-[--color-terminal-border] px-2 py-1 rounded transition-all text-sm hidden">⚙ Admin</a>
          <div class="flex items-center ml-4">
            <select id="theme-select" class="bg-[--color-terminal-bg] text-[--color-terminal-fg] border border-[--color-terminal-border] px-3 py-2 text-sm font-mono cursor-pointer hover:bg-[--color-terminal-border] focus:outline-none focus:border-[--color-terminal-accent] transition-all min-w-[120px]" onchange="switchTheme(this.value)">
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

    <style type="text/tailwindcss">
      /* Responsive header adjustments */
      @media (max-width: 768px) {
        header > div {
          @apply flex-col gap-4;
        }
        
        nav {
          @apply gap-4;
        }
        
        nav > div {
          @apply ml-0;
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
      
      // Show admin and channels links only on localhost
      (function() {
        const isLocalhost = ['localhost', '127.0.0.1', '::1'].includes(window.location.hostname);
        const adminLink = document.querySelector('.admin-link');
        const channelsLink = document.querySelector('.channels-link');
        
        if (isLocalhost) {
          if (adminLink) adminLink.style.display = 'inline-block';
          if (channelsLink) channelsLink.style.display = 'inline-block';
        }
      })();
      
      // Make switchTheme available globally
      window.switchTheme = switchTheme;
    </script>
  `
}
