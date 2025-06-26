export const title = "Theme Switcher"
export const component = "theme-switcher"

export const Default = {
  name: "Default Dropdown",
  html: `
    <div class="theme-switcher">
      <div class="webtui-select">
        <select id="theme-select">
          <option value="zinc">Zinc</option>
          <option value="catppuccin">Catppuccin</option>
          <option value="gruvbox">Gruvbox</option>
          <option value="nord">Nord</option>
          <option value="light">Light</option>
        </select>
      </div>
    </div>
  `,
  description: "Theme selection dropdown with all available themes"
}

export const ZincSelected = {
  name: "Zinc Theme Selected",
  html: `
    <div class="theme-switcher">
      <div class="webtui-select">
        <select id="theme-select">
          <option value="zinc" selected>Zinc</option>
          <option value="catppuccin">Catppuccin</option>
          <option value="gruvbox">Gruvbox</option>
          <option value="nord">Nord</option>
          <option value="light">Light</option>
        </select>
      </div>
    </div>
  `,
  description: "Theme switcher with Zinc theme selected (default dark theme)"
}

export const LightSelected = {
  name: "Light Theme Selected",
  html: `
    <div class="theme-switcher">
      <div class="webtui-select">
        <select id="theme-select">
          <option value="zinc">Zinc</option>
          <option value="catppuccin">Catppuccin</option>
          <option value="gruvbox">Gruvbox</option>
          <option value="nord">Nord</option>
          <option value="light" selected>Light</option>
        </select>
      </div>
    </div>
  `,
  description: "Theme switcher with Light theme selected"
}

export const WithButtons = {
  name: "Button Style Theme Switcher",
  html: `
    <div style="display: flex; gap: 0.5rem; flex-wrap: wrap;">
      <button class="webtui-button webtui-variant-foreground1">Zinc</button>
      <button class="webtui-button webtui-variant-background1">Catppuccin</button>
      <button class="webtui-button webtui-variant-background1">Gruvbox</button>
      <button class="webtui-button webtui-variant-background1">Nord</button>
      <button class="webtui-button webtui-variant-background1">Light</button>
    </div>
  `,
  description: "Alternative button-based theme switcher design"
}