export const title = "WebTUI Components"
export const component = "webtui"

export const Buttons = {
  name: "Buttons",
  html: `
    <div style="display: flex; gap: 1rem; align-items: center; flex-wrap: wrap;">
      <button class="webtui-button">Default Button</button>
      <button class="webtui-button webtui-variant-foreground1">Primary Button</button>
      <button class="webtui-button webtui-variant-background1">Secondary Button</button>
      <button class="webtui-button" disabled>Disabled Button</button>
    </div>
  `,
  description: "Various button styles and states available in WebTUI"
}

export const Badges = {
  name: "Badges", 
  html: `
    <div style="display: flex; gap: 1rem; align-items: center; flex-wrap: wrap;">
      <span class="webtui-badge">Default Badge</span>
      <span class="webtui-badge webtui-variant-foreground1">Primary Badge</span>
      <span class="webtui-badge webtui-variant-background1">Secondary Badge</span>
      <span class="webtui-badge webtui-variant-foreground0">Accent Badge</span>
    </div>
  `,
  description: "Badge components for labels and status indicators"
}

export const Boxes = {
  name: "Boxes",
  html: `
    <div style="display: grid; gap: 1.5rem; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));">
      <div class="webtui-box webtui-box-single">
        <div style="padding: 1rem;">
          <h3 style="margin: 0 0 0.5rem 0; color: var(--webtui-foreground1);">Single Box</h3>
          <p style="margin: 0; color: var(--webtui-foreground2);">A simple box with single border</p>
        </div>
      </div>
      <div class="webtui-box webtui-box-double">
        <div style="padding: 1rem;">
          <h3 style="margin: 0 0 0.5rem 0; color: var(--webtui-foreground1);">Double Box</h3>
          <p style="margin: 0; color: var(--webtui-foreground2);">A box with double border for emphasis</p>
        </div>
      </div>
    </div>
  `,
  description: "Container boxes with different border styles"
}

export const Forms = {
  name: "Form Elements",
  html: `
    <div style="max-width: 400px; display: flex; flex-direction: column; gap: 1rem;">
      <div>
        <label style="display: block; margin-bottom: 0.5rem; color: var(--webtui-foreground1);">Text Input</label>
        <input type="text" class="webtui-input" placeholder="Enter text here..." />
      </div>
      <div>
        <label style="display: block; margin-bottom: 0.5rem; color: var(--webtui-foreground1);">Select Dropdown</label>
        <div class="webtui-select">
          <select>
            <option>Option 1</option>
            <option>Option 2</option>
            <option>Option 3</option>
          </select>
        </div>
      </div>
      <div>
        <label style="display: block; margin-bottom: 0.5rem; color: var(--webtui-foreground1);">Textarea</label>
        <textarea class="webtui-textarea" rows="3" placeholder="Enter multiple lines of text..."></textarea>
      </div>
    </div>
  `,
  description: "Form input elements with WebTUI styling"
}

export const Typography = {
  name: "Typography",
  html: `
    <div style="max-width: 600px;">
      <h1 class="webtui-typography webtui-variant-h1" style="color: var(--webtui-foreground1);">Heading 1</h1>
      <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1);">Heading 2</h2>
      <h3 class="webtui-typography webtui-variant-h3" style="color: var(--webtui-foreground1);">Heading 3</h3>
      <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2);">
        This is body text with proper line height and spacing. WebTUI uses a monospace font family 
        for that terminal-inspired aesthetic. Perfect for developer-focused applications.
      </p>
      <p class="webtui-typography webtui-variant-caption" style="color: var(--webtui-foreground0);">
        This is caption text, typically used for metadata, timestamps, or secondary information.
      </p>
    </div>
  `,
  description: "Typography styles including headings, body text, and captions"
}