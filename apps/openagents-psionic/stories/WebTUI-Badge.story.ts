export const title = "WebTUI Badge"
export const component = "Badge"

export const Default = {
  name: "Default Badge",
  html: `<span is-="badge">Badge</span>`,
  description: "Basic badge with default styling"
}

export const Variants = {
  name: "Badge Variants",
  html: `
    <div style="display: flex; gap: 1rem; flex-wrap: wrap; align-items: center;">
      <span is-="badge" variant-="foreground0">Foreground0</span>
      <span is-="badge" variant-="foreground1">Foreground1</span>
      <span is-="badge" variant-="foreground2">Foreground2</span>
      <span is-="badge" variant-="background0">Background0</span>
      <span is-="badge" variant-="background1">Background1</span>
      <span is-="badge" variant-="background2">Background2</span>
      <span is-="badge" variant-="background3">Background3</span>
    </div>
  `,
  description: "All available badge color variants"
}

export const CapStyles = {
  name: "Badge Cap Styles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <div style="display: flex; gap: 1rem; align-items: center;">
        <span is-="badge">Default</span>
        <span is-="badge" cap-="round">Round Both</span>
        <span is-="badge" cap-="start-round">Round Start</span>
        <span is-="badge" cap-="end-round">Round End</span>
      </div>
      <div style="display: flex; gap: 1rem; align-items: center;">
        <span is-="badge" cap-="triangle">Triangle Both</span>
        <span is-="badge" cap-="start-triangle">Triangle Start</span>
        <span is-="badge" cap-="end-triangle">Triangle End</span>
      </div>
      <div style="display: flex; gap: 1rem; align-items: center;">
        <span is-="badge" cap-="slant-top">Slant Top Both</span>
        <span is-="badge" cap-="start-slant-top">Slant Top Start</span>
        <span is-="badge" cap-="end-slant-top">Slant Top End</span>
      </div>
      <div style="display: flex; gap: 1rem; align-items: center;">
        <span is-="badge" cap-="slant-bottom">Slant Bottom Both</span>
        <span is-="badge" cap-="start-slant-bottom">Slant Bottom Start</span>
        <span is-="badge" cap-="end-slant-bottom">Slant Bottom End</span>
      </div>
      <div style="display: flex; gap: 1rem; align-items: center;">
        <span is-="badge" cap-="ribbon">Ribbon Both</span>
        <span is-="badge" cap-="start-ribbon">Ribbon Start</span>
        <span is-="badge" cap-="end-ribbon">Ribbon End</span>
      </div>
    </div>
  `,
  description: "Different cap styles for badge edges"
}

export const StatusBadges = {
  name: "Status Badges",
  html: `
    <div style="display: flex; gap: 1rem; align-items: center;">
      <span is-="badge" variant-="foreground0" cap-="round">Active</span>
      <span is-="badge" variant-="foreground1" cap-="triangle">Beta</span>
      <span is-="badge" variant-="foreground2" cap-="slant-top">Deprecated</span>
      <span is-="badge" variant-="background2" cap-="ribbon">New</span>
      <span is-="badge" variant-="background3">Coming Soon</span>
    </div>
  `,
  description: "Common status badge patterns"
}

export const InlineUsage = {
  name: "Inline Badge Usage",
  html: `
    <div style="line-height: 2;">
      <p>This feature is <span is-="badge" cap-="round">new</span> and currently in <span is-="badge" variant-="foreground1">beta</span> phase.</p>
      <p>Version <span is-="badge" cap-="start-triangle">2.0.0</span> includes <span is-="badge" variant-="background2">breaking changes</span>.</p>
    </div>
  `,
  description: "Badges used inline with text"
}

export const ComplexExample = {
  name: "Complex Badge Combinations",
  html: `
    <div style="display: flex; gap: 2rem; flex-wrap: wrap;">
      <div>
        <h3 style="margin-bottom: 1rem;">Package Status</h3>
        <div style="display: flex; gap: 0.5rem;">
          <span is-="badge" variant-="foreground0" cap-="round">v1.2.3</span>
          <span is-="badge" variant-="background1" cap-="ribbon">MIT</span>
          <span is-="badge" variant-="foreground2">Weekly Downloads: 1.2k</span>
        </div>
      </div>
      <div>
        <h3 style="margin-bottom: 1rem;">Build Status</h3>
        <div style="display: flex; gap: 0.5rem;">
          <span is-="badge" variant-="foreground0" cap-="start-round">✓ Passing</span>
          <span is-="badge" variant-="background2" cap-="end-triangle">Coverage: 92%</span>
        </div>
      </div>
    </div>
  `,
  description: "Real-world badge usage examples"
}