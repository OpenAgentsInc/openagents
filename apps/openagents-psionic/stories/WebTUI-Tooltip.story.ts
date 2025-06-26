export const title = "WebTUI Tooltip"
export const component = "Tooltip"

export const Default = {
  name: "Default Tooltip",
  html: `
    <div is-="tooltip">
      <button is-="button" is-="tooltip-trigger" box-="square">Hover me</button>
      <div is-="tooltip-content" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem; white-space: nowrap;">
        This is a tooltip
      </div>
    </div>
  `,
  description: "Basic tooltip that appears on hover"
}

export const Positioning = {
  name: "Tooltip Positioning",
  html: `
    <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 4rem; padding: 4rem;">
      <div is-="tooltip">
        <button is-="button" is-="tooltip-trigger" box-="square">Top</button>
        <div is-="tooltip-content" position-="top baseline-left" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; white-space: nowrap;">
          Top tooltip
        </div>
      </div>
      
      <div is-="tooltip">
        <button is-="button" is-="tooltip-trigger" box-="square">Right</button>
        <div is-="tooltip-content" position-="right baseline-top" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; white-space: nowrap;">
          Right tooltip
        </div>
      </div>
      
      <div is-="tooltip">
        <button is-="button" is-="tooltip-trigger" box-="square">Bottom</button>
        <div is-="tooltip-content" position-="bottom baseline-left" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; white-space: nowrap;">
          Bottom tooltip
        </div>
      </div>
      
      <div is-="tooltip">
        <button is-="button" is-="tooltip-trigger" box-="square">Left</button>
        <div is-="tooltip-content" position-="left baseline-top" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; white-space: nowrap;">
          Left tooltip
        </div>
      </div>
    </div>
  `,
  description: "Tooltips in different positions"
}

export const WithDelay = {
  name: "Tooltip Delays",
  html: `
    <div style="display: flex; gap: 2rem; justify-content: center;">
      <div is-="tooltip" style="--tooltip-delay: 0s;">
        <button is-="button" is-="tooltip-trigger" box-="square">No Delay</button>
        <div is-="tooltip-content" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem;">
          Instant tooltip
        </div>
      </div>
      
      <div is-="tooltip" style="--tooltip-delay: 0.5s;">
        <button is-="button" is-="tooltip-trigger" box-="square">0.5s Delay</button>
        <div is-="tooltip-content" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem;">
          Default delay
        </div>
      </div>
      
      <div is-="tooltip" style="--tooltip-delay: 1s;">
        <button is-="button" is-="tooltip-trigger" box-="square">1s Delay</button>
        <div is-="tooltip-content" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem;">
          Longer delay
        </div>
      </div>
    </div>
  `,
  description: "Tooltips with different hover delays"
}

export const WithOffset = {
  name: "Tooltip Offset",
  html: `
    <div style="display: flex; gap: 4rem; justify-content: center; padding: 4rem;">
      <div is-="tooltip" style="--tooltip-offset-y: 0.5rem;">
        <button is-="button" is-="tooltip-trigger" box-="square">With Y Offset</button>
        <div is-="tooltip-content" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem;">
          Tooltip with gap
        </div>
      </div>
      
      <div is-="tooltip" style="--tooltip-offset-x: 0.5rem;">
        <button is-="button" is-="tooltip-trigger" box-="square">With X Offset</button>
        <div is-="tooltip-content" position-="right" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem;">
          Tooltip with gap
        </div>
      </div>
    </div>
  `,
  description: "Tooltips with custom offset from trigger"
}

export const InlineTooltips = {
  name: "Inline Tooltips",
  html: `
    <p style="line-height: 2;">
      This text contains 
      <span is-="tooltip" style="display: inline-block;">
        <span is-="tooltip-trigger" style="text-decoration: underline; cursor: help;">inline tooltips</span>
        <span is-="tooltip-content" style="background: var(--background3); color: var(--foreground0); padding: 0.25rem 0.5rem; font-size: 0.9em;">
          Additional information
        </span>
      </span>
      that provide extra context when 
      <span is-="tooltip" style="display: inline-block;">
        <span is-="tooltip-trigger" style="text-decoration: underline; cursor: help;">hovered</span>
        <span is-="tooltip-content" position-="bottom" style="background: var(--background3); color: var(--foreground0); padding: 0.25rem 0.5rem; font-size: 0.9em;">
          Like this one!
        </span>
      </span>.
    </p>
  `,
  description: "Tooltips used inline with text"
}

export const IconTooltips = {
  name: "Icon Tooltips",
  html: `
    <div style="display: flex; gap: 2rem; align-items: center;">
      <div is-="tooltip">
        <button is-="button" is-="tooltip-trigger" size-="small" box-="round">?</button>
        <div is-="tooltip-content" position-="right" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem; max-width: 200px;">
          This help tooltip provides additional information about this feature.
        </div>
      </div>
      
      <div is-="tooltip">
        <button is-="button" is-="tooltip-trigger" size-="small" box-="square">i</button>
        <div is-="tooltip-content" position-="bottom" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem;">
          Information tooltip
        </div>
      </div>
      
      <div is-="tooltip">
        <button is-="button" is-="tooltip-trigger" size-="small">!</button>
        <div is-="tooltip-content" position-="top" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem 1rem;">
          Warning tooltip
        </div>
      </div>
    </div>
  `,
  description: "Small icon buttons with tooltips"
}

export const FormTooltips = {
  name: "Form Field Tooltips",
  html: `
    <form style="max-width: 400px;">
      <div style="margin-bottom: 1rem;">
        <label style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem;">
          Username
          <span is-="tooltip" style="display: inline-block;">
            <span is-="tooltip-trigger" style="font-size: 0.8em; cursor: help;">[?]</span>
            <span is-="tooltip-content" position-="right" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; font-size: 0.9em; max-width: 200px;">
              Must be 3-20 characters, alphanumeric only
            </span>
          </span>
        </label>
        <input is-="input" box-="square" placeholder="Enter username">
      </div>
      
      <div style="margin-bottom: 1rem;">
        <label style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem;">
          Password
          <span is-="tooltip" style="display: inline-block;">
            <span is-="tooltip-trigger" style="font-size: 0.8em; cursor: help;">[?]</span>
            <span is-="tooltip-content" position-="right" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; font-size: 0.9em; max-width: 200px;">
              Minimum 8 characters, must include uppercase, lowercase, and numbers
            </span>
          </span>
        </label>
        <input is-="input" type="password" box-="square" placeholder="Enter password">
      </div>
    </form>
  `,
  description: "Form fields with helpful tooltips"
}

export const ComplexTooltip = {
  name: "Complex Tooltip Content",
  html: `
    <div style="display: flex; justify-content: center; padding: 4rem;">
      <div is-="tooltip">
        <button is-="button" is-="tooltip-trigger" box-="square">View Details</button>
        <div is-="tooltip-content" position-="bottom" style="background: var(--background0); border: 1px solid var(--foreground2); padding: 1rem; min-width: 250px;">
          <h4 style="margin-bottom: 0.5rem;">Component Status</h4>
          <table style="font-size: 0.9em;">
            <tr>
              <td style="padding-right: 1rem;">Version:</td>
              <td>2.1.0</td>
            </tr>
            <tr>
              <td style="padding-right: 1rem;">Last Updated:</td>
              <td>Dec 17, 2024</td>
            </tr>
            <tr>
              <td style="padding-right: 1rem;">Status:</td>
              <td><span is-="badge" variant-="foreground0">Active</span></td>
            </tr>
          </table>
          <div style="margin-top: 0.5rem; padding-top: 0.5rem; border-top: 1px solid var(--foreground2);">
            <button is-="button" size-="small" box-="square" style="width: 100%;">More Info</button>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Tooltip with complex structured content"
}

export const TableTooltips = {
  name: "Table with Tooltips",
  html: `
    <table>
      <thead>
        <tr>
          <th>
            <span is-="tooltip">
              <span is-="tooltip-trigger">Status</span>
              <span is-="tooltip-content" position-="top" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; font-size: 0.9em;">
                Current operational status
              </span>
            </span>
          </th>
          <th>
            <span is-="tooltip">
              <span is-="tooltip-trigger">CPU %</span>
              <span is-="tooltip-content" position-="top" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; font-size: 0.9em;">
                Average CPU usage over last 5 minutes
              </span>
            </span>
          </th>
          <th>Memory</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td>
            <span is-="tooltip">
              <span is-="badge" is-="tooltip-trigger" variant-="foreground0">Running</span>
              <span is-="tooltip-content" position-="right" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; font-size: 0.9em;">
                Uptime: 42 days
              </span>
            </span>
          </td>
          <td>23%</td>
          <td>4.2 GB</td>
        </tr>
        <tr>
          <td>
            <span is-="tooltip">
              <span is-="badge" is-="tooltip-trigger" variant-="background2">Idle</span>
              <span is-="tooltip-content" position-="right" style="background: var(--background3); color: var(--foreground0); padding: 0.5rem; font-size: 0.9em;">
                Last active: 2 hours ago
              </span>
            </span>
          </td>
          <td>2%</td>
          <td>1.1 GB</td>
        </tr>
      </tbody>
    </table>
  `,
  description: "Table cells with informative tooltips"
}