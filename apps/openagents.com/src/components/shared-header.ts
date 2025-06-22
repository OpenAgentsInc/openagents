import { html } from "@openagentsinc/psionic"

interface HeaderOptions {
  current?: string
}

export function sharedHeader({ current }: HeaderOptions = {}) {
  return html`
    <header class="oa-header">
      <div class="oa-header-content">
        <div class="oa-header-brand">
          <h1 class="oa-header-title">OpenAgents</h1>
        </div>
        
        <nav class="oa-header-nav">
          <a href="/channels" class="oa-header-nav-link channels-link ${
    current === "channels" ? "active" : ""
  }" style="display: none;">Channels</a>
          <a href="/docs" class="oa-header-nav-link ${current === "docs" ? "active" : ""}">Docs</a>
          <a href="/blog" class="oa-header-nav-link ${current === "blog" ? "active" : ""}">Blog</a>
          <a href="/admin" class="oa-header-nav-link admin-link ${
    current === "admin" ? "active" : ""
  }" style="display: none;">Admin</a>
        </nav>
        
        <div class="oa-header-actions">
          <div class="oa-dropdown">
            <button class="oa-dropdown-trigger" onclick="toggleThemeDropdown()">
              <span id="current-theme">Zinc</span>
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
              </svg>
            </button>
            <div class="oa-dropdown-menu" id="theme-dropdown">
              <div class="oa-dropdown-item" onclick="selectTheme('zinc')">Zinc</div>
              <div class="oa-dropdown-item" onclick="selectTheme('ayu')">Ayu</div>
              <div class="oa-dropdown-item" onclick="selectTheme('catppuccin')">Catppuccin</div>
              <div class="oa-dropdown-item" onclick="selectTheme('flexoki')">Flexoki</div>
              <div class="oa-dropdown-item" onclick="selectTheme('gruvbox')">Gruvbox</div>
              <div class="oa-dropdown-item" onclick="selectTheme('monokai')">Monokai</div>
              <div class="oa-dropdown-item" onclick="selectTheme('nord')">Nord</div>
              <div class="oa-dropdown-item" onclick="selectTheme('onedark')">One Dark</div>
              <div class="oa-dropdown-item" onclick="selectTheme('tokyonight')">Tokyo Night</div>
              <div class="oa-dropdown-item" onclick="selectTheme('tron')">Tron</div>
            </div>
          </div>
        </div>
        
        <button class="oa-header-mobile-button" onclick="toggleMobileMenu()">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"></path>
          </svg>
        </button>
      </div>
    </header>

    <script>
      // Theme dropdown functionality
      function toggleThemeDropdown() {
        const dropdown = document.querySelector('.oa-dropdown');
        if (dropdown) {
          dropdown.classList.toggle('open');
        }
      }
      
      function selectTheme(theme) {
        // Update dropdown text
        const currentTheme = document.getElementById('current-theme');
        if (currentTheme) {
          currentTheme.textContent = theme.charAt(0).toUpperCase() + theme.slice(1);
        }
        
        // Close dropdown
        const dropdown = document.querySelector('.oa-dropdown');
        if (dropdown) {
          dropdown.classList.remove('open');
        }
        
        // Switch theme
        switchTheme(theme);
      }
      
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
        const currentThemeEl = document.getElementById('current-theme');
        
        if (currentThemeEl) {
          currentThemeEl.textContent = savedTheme.charAt(0).toUpperCase() + savedTheme.slice(1);
        }
        
        // Handle old 'light' or 'zinc-light' values for backwards compatibility
        if (savedTheme === 'light' || savedTheme === 'zinc-light') {
          switchTheme('zinc');
        } else {
          switchTheme(savedTheme);
        }
      })();
      
      // Show admin and channels links only on localhost
      (function() {
        const isLocalhost = ['localhost', '127.0.0.1', '::1'].includes(window.location.hostname);
        const adminLink = document.querySelector('.admin-link');
        const channelsLink = document.querySelector('.channels-link');
        
        if (isLocalhost) {
          if (adminLink) adminLink.style.display = 'block';
          if (channelsLink) channelsLink.style.display = 'block';
        }
      })();
      
      // Mobile menu toggle
      function toggleMobileMenu() {
        // TODO: Implement mobile menu
      }
      
      // Close dropdown when clicking outside
      document.addEventListener('click', function(event) {
        const dropdown = document.querySelector('.oa-dropdown');
        if (dropdown && !dropdown.contains(event.target)) {
          dropdown.classList.remove('open');
        }
      });
      
      // Make functions available globally
      window.switchTheme = switchTheme;
      window.toggleThemeDropdown = toggleThemeDropdown;
      window.selectTheme = selectTheme;
      window.toggleMobileMenu = toggleMobileMenu;
    </script>
  `
}
