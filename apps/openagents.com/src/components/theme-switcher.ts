import { html } from "@openagentsinc/psionic"

export function themeSwitcher(): string {
  return html`
    <div class="theme-switcher">
      <select id="theme-select" onchange="switchTheme(this.value)" style="font-family: monospace;">
        <option value="zinc">Zinc Dark</option>
        <option value="zinc-light">Zinc Light</option>
        <option value="catppuccin">Catppuccin</option>
        <option value="gruvbox">Gruvbox</option>
        <option value="nord">Nord</option>
      </select>
    </div>
    
    <script>
      // Theme switching functionality
      function switchTheme(theme) {
        console.log('Switching to theme:', theme);
        
        // Remove existing theme classes
        document.body.classList.remove('theme-zinc', 'theme-zinc-light', 'theme-catppuccin', 'theme-gruvbox', 'theme-nord');
        
        // Add new theme class
        document.body.classList.add('theme-' + theme);
        console.log('Applied theme class: theme-' + theme);
        
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
          // Handle old 'light' value for backwards compatibility
          if (savedTheme === 'light') {
            themeSelect.value = 'zinc-light';
            switchTheme('zinc-light');
          } else {
            themeSelect.value = savedTheme;
            switchTheme(savedTheme);
          }
        }
      })();
      
      // Make switchTheme available globally
      window.switchTheme = switchTheme;
    </script>
  `
}
