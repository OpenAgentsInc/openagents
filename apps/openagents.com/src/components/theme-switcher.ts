import { html } from "@openagentsinc/psionic"

export function themeSwitcher(): string {
  return html`
    <div class="theme-switcher">
      <div class="webtui-select">
        <select id="theme-select" onchange="switchTheme(this.value)">
          <option value="zinc">Zinc</option>
          <option value="catppuccin">Catppuccin</option>
          <option value="gruvbox">Gruvbox</option>
          <option value="nord">Nord</option>
          <option value="light">Light</option>
        </select>
      </div>
    </div>
    
    <script>
      // Theme switching functionality
      function switchTheme(theme) {
        // Remove existing theme classes
        document.body.classList.remove('webtui-theme-zinc', 'webtui-theme-catppuccin', 'webtui-theme-gruvbox', 'webtui-theme-nord', 'webtui-theme-light');
        
        // Add new theme class
        document.body.classList.add('webtui-theme-' + theme);
        
        // Save theme preference
        localStorage.setItem('openagents-theme', theme);
        
        // Load theme CSS if not zinc (default)
        const existingThemeLink = document.getElementById('theme-css');
        if (existingThemeLink) {
          existingThemeLink.remove();
        }
        
        if (theme !== 'zinc') {
          const link = document.createElement('link');
          link.id = 'theme-css';
          link.rel = 'stylesheet';
          link.href = '/theme-' + theme + '.css';
          document.head.appendChild(link);
        }
      }
      
      // Initialize theme on page load
      (function() {
        const savedTheme = localStorage.getItem('openagents-theme') || 'zinc';
        const themeSelect = document.getElementById('theme-select');
        
        if (themeSelect) {
          themeSelect.value = savedTheme;
          switchTheme(savedTheme);
        }
      })();
      
      // Make switchTheme available globally
      window.switchTheme = switchTheme;
    </script>
  `
}
