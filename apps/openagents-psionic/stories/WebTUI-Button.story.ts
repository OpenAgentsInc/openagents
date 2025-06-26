export const title = "WebTUI Button"
export const component = "Button"

export const Default = {
  name: "Default Button",
  html: `<button is-="button">Default Button</button>`,
  description: "Basic button with default styling"
}

export const AsLink = {
  name: "Button as Link",
  html: `<a is-="button" href="#">Link Button</a>`,
  description: "Button styling applied to anchor element"
}

export const Variants = {
  name: "Button Variants",
  html: `
    <div style="display: flex; gap: 1rem; flex-wrap: wrap; align-items: center;">
      <button is-="button" variant-="foreground0">Foreground0</button>
      <button is-="button" variant-="foreground1">Foreground1</button>
      <button is-="button" variant-="foreground2">Foreground2</button>
      <button is-="button" variant-="background0">Background0</button>
      <button is-="button" variant-="background1">Background1</button>
      <button is-="button" variant-="background2">Background2</button>
      <button is-="button" variant-="background3">Background3</button>
    </div>
  `,
  description: "All available button color variants"
}

export const Sizes = {
  name: "Button Sizes",
  html: `
    <div style="display: flex; gap: 1rem; align-items: center;">
      <button is-="button" size-="small">Small</button>
      <button is-="button">Default</button>
      <button is-="button" size-="large">Large</button>
    </div>
  `,
  description: "Button size variations"
}

export const BoxStyles = {
  name: "Button Box Styles",
  html: `
    <div style="display: flex; gap: 1rem; align-items: center;">
      <button is-="button" box-="square">Square Box</button>
      <button is-="button" box-="round">Round Box</button>
      <button is-="button" box-="double">Double Box</button>
    </div>
  `,
  description: "Different ASCII box border styles"
}

export const BoxVariants = {
  name: "Box Style with Variants",
  html: `
    <div style="display: flex; gap: 1rem; flex-wrap: wrap;">
      <button is-="button" box-="square" variant-="foreground0">Square FG0</button>
      <button is-="button" box-="round" variant-="foreground1">Round FG1</button>
      <button is-="button" box-="double" variant-="background2">Double BG2</button>
    </div>
  `,
  description: "Box styles combined with color variants"
}

export const States = {
  name: "Button States",
  html: `
    <div style="display: flex; gap: 1rem; align-items: center;">
      <button is-="button">Normal</button>
      <button is-="button" autofocus>Focused</button>
      <button is-="button" disabled>Disabled</button>
    </div>
  `,
  description: "Different button states (hover and active states are interactive)"
}

export const ComplexExample = {
  name: "Complex Button Layout",
  html: `
    <div style="display: flex; flex-direction: column; gap: 2rem;">
      <div style="display: flex; gap: 1rem;">
        <button is-="button" box-="square" size-="large" variant-="foreground0">Save</button>
        <button is-="button" box-="square" size-="large" variant-="background1">Cancel</button>
      </div>
      <div style="display: flex; gap: 0.5rem;">
        <button is-="button" size-="small">[Y]es</button>
        <button is-="button" size-="small">[N]o</button>
        <button is-="button" size-="small">[C]ancel</button>
      </div>
    </div>
  `,
  description: "Complex button layouts for dialogs and forms"
}