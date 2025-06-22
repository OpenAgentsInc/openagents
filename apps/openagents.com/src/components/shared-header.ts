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
        
        
        <button class="oa-header-mobile-button" onclick="toggleMobileMenu()">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"></path>
          </svg>
        </button>
      </div>
    </header>

    <script>
      
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
      
      // Make functions available globally
      window.toggleMobileMenu = toggleMobileMenu;
    </script>
  `
}
