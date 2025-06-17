export const title = "WebTUI Popover"
export const component = "Popover"

export const Default = {
  name: "Default Popover",
  html: `
    <details is-="popover">
      <summary>
        <button is-="button" box-="square">Click me</button>
      </summary>
      <div style="background: var(--background1); padding: 1rem; border: 1px solid var(--foreground2);">
        <p>This is the popover content.</p>
        <p>It appears below the trigger by default.</p>
      </div>
    </details>
  `,
  description: "Basic popover with default bottom-left positioning"
}

export const Positioning = {
  name: "Popover Positioning",
  html: `
    <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 4rem; padding: 4rem;">
      <details is-="popover" position-="top left">
        <summary>
          <button is-="button" box-="square">Top Left</button>
        </summary>
        <div style="background: var(--background1); padding: 0.5rem; border: 1px solid var(--foreground2); white-space: nowrap;">
          Top Left Position
        </div>
      </details>
      
      <details is-="popover" position-="top baseline-left">
        <summary>
          <button is-="button" box-="square">Top Center</button>
        </summary>
        <div style="background: var(--background1); padding: 0.5rem; border: 1px solid var(--foreground2); white-space: nowrap;">
          Top Center Position
        </div>
      </details>
      
      <details is-="popover" position-="top right">
        <summary>
          <button is-="button" box-="square">Top Right</button>
        </summary>
        <div style="background: var(--background1); padding: 0.5rem; border: 1px solid var(--foreground2); white-space: nowrap;">
          Top Right Position
        </div>
      </details>
      
      <details is-="popover" position-="baseline-bottom left">
        <summary>
          <button is-="button" box-="square">Left</button>
        </summary>
        <div style="background: var(--background1); padding: 0.5rem; border: 1px solid var(--foreground2); white-space: nowrap;">
          Left Position
        </div>
      </details>
      
      <details is-="popover">
        <summary>
          <button is-="button" box-="square">Center</button>
        </summary>
        <div style="background: var(--background1); padding: 0.5rem; border: 1px solid var(--foreground2); white-space: nowrap;">
          Default (Bottom Left)
        </div>
      </details>
      
      <details is-="popover" position-="baseline-bottom right">
        <summary>
          <button is-="button" box-="square">Right</button>
        </summary>
        <div style="background: var(--background1); padding: 0.5rem; border: 1px solid var(--foreground2); white-space: nowrap;">
          Right Position
        </div>
      </details>
      
      <details is-="popover" position-="bottom left">
        <summary>
          <button is-="button" box-="square">Bottom Left</button>
        </summary>
        <div style="background: var(--background1); padding: 0.5rem; border: 1px solid var(--foreground2); white-space: nowrap;">
          Bottom Left Position
        </div>
      </details>
      
      <details is-="popover" position-="bottom baseline-left">
        <summary>
          <button is-="button" box-="square">Bottom Center</button>
        </summary>
        <div style="background: var(--background1); padding: 0.5rem; border: 1px solid var(--foreground2); white-space: nowrap;">
          Bottom Center Position
        </div>
      </details>
      
      <details is-="popover" position-="bottom right">
        <summary>
          <button is-="button" box-="square">Bottom Right</button>
        </summary>
        <div style="background: var(--background1); padding: 0.5rem; border: 1px solid var(--foreground2); white-space: nowrap;">
          Bottom Right Position
        </div>
      </details>
    </div>
  `,
  description: "All popover positioning options"
}

export const WithOffset = {
  name: "Popover with Offset",
  html: `
    <div style="display: flex; gap: 4rem; justify-content: center; padding: 4rem;">
      <details is-="popover" style="--popover-offset-y: 0.5rem;">
        <summary>
          <button is-="button" box-="square">With Y Offset</button>
        </summary>
        <div style="background: var(--background1); padding: 1rem; border: 1px solid var(--foreground2);">
          This popover has vertical offset
        </div>
      </details>
      
      <details is-="popover" position-="right" style="--popover-offset-x: 0.5rem;">
        <summary>
          <button is-="button" box-="square">With X Offset</button>
        </summary>
        <div style="background: var(--background1); padding: 1rem; border: 1px solid var(--foreground2);">
          This popover has horizontal offset
        </div>
      </details>
    </div>
  `,
  description: "Popovers with custom offset values"
}

export const MenuExample = {
  name: "Dropdown Menu",
  html: `
    <details is-="popover">
      <summary>
        <button is-="button" box-="square">
          Menu <span style="font-size: 0.8em;">▼</span>
        </button>
      </summary>
      <div style="background: var(--background0); border: 1px solid var(--foreground2); min-width: 200px;">
        <div box-="square" style="padding: 0;">
          <a href="#" style="display: block; padding: 0.5rem 1rem; color: inherit; text-decoration: none;">Profile</a>
          <a href="#" style="display: block; padding: 0.5rem 1rem; color: inherit; text-decoration: none;">Settings</a>
          <a href="#" style="display: block; padding: 0.5rem 1rem; color: inherit; text-decoration: none;">Help</a>
          <span is-="separator" style="width: 100%;"></span>
          <a href="#" style="display: block; padding: 0.5rem 1rem; color: inherit; text-decoration: none;">Sign Out</a>
        </div>
      </div>
    </details>
  `,
  description: "Popover used as dropdown menu"
}

export const TooltipStyle = {
  name: "Tooltip Style Popover",
  html: `
    <div style="display: flex; gap: 2rem; align-items: center; justify-content: center; padding: 4rem;">
      <details is-="popover" position-="top baseline-left">
        <summary>
          <span style="text-decoration: underline; cursor: help;">Hover for info</span>
        </summary>
        <div style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem; font-size: 0.9em; max-width: 200px;">
          This is additional information that appears on interaction.
        </div>
      </details>
      
      <details is-="popover" position-="bottom baseline-left">
        <summary>
          <button is-="button" size-="small">?</button>
        </summary>
        <div style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem; font-size: 0.9em;">
          Help text appears here
        </div>
      </details>
    </div>
  `,
  description: "Popovers styled as tooltips"
}

export const FormPopover = {
  name: "Form in Popover",
  html: `
    <details is-="popover">
      <summary>
        <button is-="button" box-="square">Add Item</button>
      </summary>
      <div style="background: var(--background0); border: 1px solid var(--foreground2); padding: 1.5rem; min-width: 300px;">
        <h4 style="margin-bottom: 1rem;">Quick Add</h4>
        <form style="display: flex; flex-direction: column; gap: 1rem;">
          <input is-="input" placeholder="Item name" box-="square">
          <textarea is-="textarea" placeholder="Description" rows="3" box-="square"></textarea>
          <div style="display: flex; gap: 0.5rem;">
            <button is-="button" box-="square" type="submit">Add</button>
            <button is-="button" box-="square" variant-="background1" type="button">Cancel</button>
          </div>
        </form>
      </div>
    </details>
  `,
  description: "Popover containing a form"
}

export const NestedPopovers = {
  name: "Nested Popovers",
  html: `
    <details is-="popover">
      <summary>
        <button is-="button" box-="square">Options</button>
      </summary>
      <div style="background: var(--background0); border: 1px solid var(--foreground2); padding: 1rem;">
        <p>Main popover content</p>
        <details is-="popover" position-="right">
          <summary>
            <button is-="button" size-="small">More</button>
          </summary>
          <div style="background: var(--background1); border: 1px solid var(--foreground2); padding: 1rem;">
            <p>Nested popover content</p>
          </div>
        </details>
      </div>
    </details>
  `,
  description: "Popover containing another popover"
}

export const ComplexExample = {
  name: "User Profile Popover",
  html: `
    <div style="display: flex; justify-content: flex-end; padding: 2rem;">
      <details is-="popover" position-="bottom baseline-right">
        <summary>
          <button is-="button" box-="round">
            <span style="display: flex; align-items: center; gap: 0.5rem;">
              <span style="display: inline-block; width: 2ch; height: 1lh; background: var(--foreground2); border-radius: 50%;"></span>
              John Doe
            </span>
          </button>
        </summary>
        <div style="background: var(--background0); border: 1px solid var(--foreground2); min-width: 250px;">
          <div style="padding: 1rem; border-bottom: 1px solid var(--foreground2);">
            <div style="display: flex; align-items: center; gap: 1rem;">
              <div style="width: 4ch; height: 2lh; background: var(--foreground2); border-radius: 50%;"></div>
              <div>
                <div style="font-weight: bold;">John Doe</div>
                <div style="font-size: 0.9em; color: var(--foreground2);">john.doe@example.com</div>
              </div>
            </div>
          </div>
          <div style="padding: 0.5rem 0;">
            <a href="#" style="display: block; padding: 0.5rem 1rem; color: inherit; text-decoration: none;">View Profile</a>
            <a href="#" style="display: block; padding: 0.5rem 1rem; color: inherit; text-decoration: none;">Account Settings</a>
            <a href="#" style="display: block; padding: 0.5rem 1rem; color: inherit; text-decoration: none;">Preferences</a>
          </div>
          <div style="padding: 0.5rem 1rem; border-top: 1px solid var(--foreground2);">
            <button is-="button" box-="square" style="width: 100%;">Sign Out</button>
          </div>
        </div>
      </details>
    </div>
  `,
  description: "Complex user profile popover"
}