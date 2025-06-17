export const title = "WebTUI Input"
export const component = "Input"

export const Default = {
  name: "Default Input",
  html: `<input is-="input" placeholder="Enter text...">`,
  description: "Basic text input with default styling"
}

export const Sizes = {
  name: "Input Sizes",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <input is-="input" size-="small" placeholder="Small input">
      <input is-="input" placeholder="Default input">
      <input is-="input" size-="large" placeholder="Large input">
    </div>
  `,
  description: "Input size variations"
}

export const BoxStyles = {
  name: "Input Box Styles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <input is-="input" box-="square" placeholder="Square box">
      <input is-="input" box-="round" placeholder="Round box">
      <input is-="input" box-="double" placeholder="Double box">
    </div>
  `,
  description: "Different ASCII box border styles"
}

export const InputTypes = {
  name: "Input Types",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <input is-="input" type="text" placeholder="Text input">
      <input is-="input" type="email" placeholder="email@example.com">
      <input is-="input" type="password" placeholder="Password">
      <input is-="input" type="number" placeholder="Number">
      <input is-="input" type="tel" placeholder="Phone number">
      <input is-="input" type="url" placeholder="https://example.com">
      <input is-="input" type="search" placeholder="Search...">
    </div>
  `,
  description: "Various HTML5 input types with WebTUI styling"
}

export const States = {
  name: "Input States",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <input is-="input" placeholder="Normal state">
      <input is-="input" placeholder="With value" value="Filled input">
      <input is-="input" placeholder="Focused state" autofocus>
      <input is-="input" placeholder="Disabled state" disabled>
      <input is-="input" placeholder="Readonly state" readonly value="Read only">
    </div>
  `,
  description: "Different input states"
}

export const FormExample = {
  name: "Form Example",
  html: `
    <form style="display: flex; flex-direction: column; gap: 1.5rem; max-width: 48ch;">
      <div>
        <label for="username" style="display: block; margin-bottom: 0.5rem;">Username</label>
        <input is-="input" box-="square" id="username" placeholder="Enter username">
      </div>
      <div>
        <label for="email" style="display: block; margin-bottom: 0.5rem;">Email</label>
        <input is-="input" box-="square" type="email" id="email" placeholder="email@example.com">
      </div>
      <div>
        <label for="password" style="display: block; margin-bottom: 0.5rem;">Password</label>
        <input is-="input" box-="square" type="password" id="password" placeholder="Enter password">
      </div>
      <div style="display: flex; gap: 1rem;">
        <button is-="button" box-="square" type="submit">Submit</button>
        <button is-="button" box-="square" variant-="background1" type="reset">Reset</button>
      </div>
    </form>
  `,
  description: "Complete form example with inputs and buttons"
}

export const InlineInputs = {
  name: "Inline Input Layouts",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <div style="display: flex; gap: 1rem; align-items: center;">
        <label>Search:</label>
        <input is-="input" size-="small" box-="square" type="search" placeholder="Type to search...">
        <button is-="button" size-="small" box-="square">Go</button>
      </div>
      <div style="display: flex; gap: 0.5rem; align-items: center;">
        <input is-="input" size-="small" style="min-width: 8ch; width: 8ch;" placeholder="DD">
        <span>/</span>
        <input is-="input" size-="small" style="min-width: 8ch; width: 8ch;" placeholder="MM">
        <span>/</span>
        <input is-="input" size-="small" style="min-width: 12ch; width: 12ch;" placeholder="YYYY">
      </div>
    </div>
  `,
  description: "Inline input layouts for search and date entry"
}