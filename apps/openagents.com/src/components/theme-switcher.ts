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
        console.log('Switching to theme:', theme);
        
        // Remove existing theme classes
        document.body.classList.remove('webtui-theme-zinc', 'webtui-theme-zinc-light', 'webtui-theme-catppuccin', 'webtui-theme-gruvbox', 'webtui-theme-nord');
        
        // Add new theme class (light theme is a variant of zinc)
        if (theme === 'light') {
          document.body.classList.add('webtui-theme-zinc-light');
          console.log('Applied light theme class: webtui-theme-zinc-light');
        } else {
          document.body.classList.add('webtui-theme-' + theme);
          console.log('Applied theme class: webtui-theme-' + theme);
        }
        
        // Save theme preference
        localStorage.setItem('openagents-theme', theme);
        
        // Log current body classes for debugging
        console.log('Current body classes:', document.body.className);
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
