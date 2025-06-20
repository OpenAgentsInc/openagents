import { html } from "@openagentsinc/psionic"

export function themeSwitcher(): string {
  return html`
    <div class="theme-switcher">
      <select id="theme-select" onchange="switchTheme(this.value)" style="font-family: 'Berkeley Mono', ui-monospace, 'Cascadia Code', 'Source Code Pro', Menlo, Consolas, 'DejaVu Sans Mono', monospace;">
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
      
      // Make switchTheme available globally
      window.switchTheme = switchTheme;
    </script>
  `
}
