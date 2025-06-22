import { html } from "@openagentsinc/psionic"

interface HeaderOptions {
  current?: string
}

export function sharedHeader({ current }: HeaderOptions = {}) {
  return html`
    <header class="oa-header">
      <div class="oa-header-content">
        <div class="oa-header-brand">
          <a href="/" class="oa-header-title" style="text-decoration: none; color: inherit;">OpenAgents</a>
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
      
    </script>
  `
}
