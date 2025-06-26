export const title = "WebTUI Separator"
export const component = "Separator"

export const Default = {
  name: "Default Separator",
  html: `
    <div style="width: 100%;">
      <span is-="separator" style="width: 100%;"></span>
    </div>
  `,
  description: "Basic horizontal separator"
}

export const Directions = {
  name: "Separator Directions",
  html: `
    <div style="display: flex; flex-direction: column; gap: 2rem;">
      <div>
        <p>Horizontal (default):</p>
        <span is-="separator" style="width: 100%;"></span>
      </div>
      
      <div>
        <p>Horizontal explicit:</p>
        <span is-="separator" direction-="horizontal" style="width: 100%;"></span>
      </div>
      
      <div style="display: flex; align-items: center; gap: 1rem;">
        <span>Left content</span>
        <span is-="separator" direction-="vertical" style="height: 3rem;"></span>
        <span>Right content</span>
      </div>
    </div>
  `,
  description: "Horizontal and vertical separators"
}

export const Variants = {
  name: "Separator Variants",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
      <div>
        <p style="margin-bottom: 0.5rem;">Foreground0 (default):</p>
        <span is-="separator" variant-="foreground0" style="width: 100%;"></span>
      </div>
      
      <div>
        <p style="margin-bottom: 0.5rem;">Foreground1:</p>
        <span is-="separator" variant-="foreground1" style="width: 100%;"></span>
      </div>
      
      <div>
        <p style="margin-bottom: 0.5rem;">Foreground2:</p>
        <span is-="separator" variant-="foreground2" style="width: 100%;"></span>
      </div>
    </div>
  `,
  description: "Different separator color variants"
}

export const InContent = {
  name: "Separators in Content",
  html: `
    <article style="max-width: 48ch;">
      <h2>Article Title</h2>
      <p>This is the first paragraph of content. It contains some introductory text.</p>
      
      <span is-="separator" style="width: 100%; margin: 1.5rem 0;"></span>
      
      <h3>Section 1</h3>
      <p>This section contains the main content. Lorem ipsum dolor sit amet.</p>
      
      <span is-="separator" variant-="foreground2" style="width: 100%; margin: 1.5rem 0;"></span>
      
      <h3>Section 2</h3>
      <p>Another section with different content. The separator helps visually divide sections.</p>
      
      <span is-="separator" variant-="foreground1" style="width: 100%; margin: 1.5rem 0;"></span>
      
      <footer>
        <p style="font-size: 0.9em; color: var(--foreground2);">Published: December 2024</p>
      </footer>
    </article>
  `,
  description: "Separators used to divide content sections"
}

export const NavigationExample = {
  name: "Navigation with Separators",
  html: `
    <nav style="display: flex; align-items: center; gap: 1rem; flex-wrap: wrap;">
      <a href="#">Home</a>
      <span is-="separator" direction-="vertical" style="height: 1rem;"></span>
      <a href="#">About</a>
      <span is-="separator" direction-="vertical" style="height: 1rem;"></span>
      <a href="#">Services</a>
      <span is-="separator" direction-="vertical" style="height: 1rem;"></span>
      <a href="#">Contact</a>
    </nav>
  `,
  description: "Vertical separators in navigation"
}

export const FormSections = {
  name: "Form with Separators",
  html: `
    <form style="max-width: 48ch;">
      <h3>Personal Information</h3>
      <div style="display: flex; flex-direction: column; gap: 1rem; margin-bottom: 1.5rem;">
        <input is-="input" placeholder="First Name">
        <input is-="input" placeholder="Last Name">
        <input is-="input" type="email" placeholder="Email">
      </div>
      
      <span is-="separator" style="width: 100%; margin: 2rem 0;"></span>
      
      <h3>Account Settings</h3>
      <div style="display: flex; flex-direction: column; gap: 1rem; margin-bottom: 1.5rem;">
        <input is-="input" placeholder="Username">
        <input is-="input" type="password" placeholder="Password">
        <input is-="input" type="password" placeholder="Confirm Password">
      </div>
      
      <span is-="separator" variant-="foreground2" style="width: 100%; margin: 2rem 0;"></span>
      
      <h3>Preferences</h3>
      <div style="display: flex; flex-direction: column; gap: 0.5rem;">
        <label>
          <input type="checkbox">
          Subscribe to newsletter
        </label>
        <label>
          <input type="checkbox">
          Enable notifications
        </label>
      </div>
    </form>
  `,
  description: "Separators dividing form sections"
}

export const ComplexLayout = {
  name: "Complex Layout with Separators",
  html: `
    <div style="display: flex; gap: 2rem;">
      <aside style="flex: 0 0 200px;">
        <h4>Sidebar</h4>
        <ul style="list-style: none; padding: 0;">
          <li>Item 1</li>
          <li>Item 2</li>
          <li>Item 3</li>
        </ul>
      </aside>
      
      <span is-="separator" direction-="vertical" style="height: 200px;"></span>
      
      <main style="flex: 1;">
        <h2>Main Content</h2>
        <p>This is the main content area.</p>
        
        <span is-="separator" style="width: 100%; margin: 1.5rem 0;"></span>
        
        <div style="display: flex; gap: 1rem;">
          <button is-="button" box-="square">Action 1</button>
          <span is-="separator" direction-="vertical" style="height: 3lh;"></span>
          <button is-="button" box-="square" variant-="background1">Action 2</button>
        </div>
      </main>
    </div>
  `,
  description: "Complex layout using both horizontal and vertical separators"
}