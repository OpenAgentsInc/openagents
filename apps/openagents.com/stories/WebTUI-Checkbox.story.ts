export const title = "WebTUI Checkbox"
export const component = "Checkbox"

export const Default = {
  name: "Default Checkbox",
  html: `<input type="checkbox">`,
  description: "Basic checkbox with WebTUI styling"
}

export const WithLabel = {
  name: "Checkbox with Label",
  html: `
    <label>
      <input type="checkbox">
      Accept terms and conditions
    </label>
  `,
  description: "Checkbox with associated label"
}

export const States = {
  name: "Checkbox States",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <label>
        <input type="checkbox">
        Unchecked
      </label>
      <label>
        <input type="checkbox" checked>
        Checked
      </label>
      <label>
        <input type="checkbox" disabled>
        Disabled unchecked
      </label>
      <label>
        <input type="checkbox" checked disabled>
        Disabled checked
      </label>
    </div>
  `,
  description: "Different checkbox states"
}

export const BoxStyles = {
  name: "Checkbox with Box Styles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <label>
        <input type="checkbox" box-="square">
        Square box checkbox
      </label>
      <label>
        <input type="checkbox" box-="round">
        Round box checkbox
      </label>
      <label>
        <input type="checkbox" box-="double">
        Double box checkbox
      </label>
    </div>
  `,
  description: "Checkboxes with box border styles"
}

export const CheckboxGroup = {
  name: "Checkbox Group",
  html: `
    <fieldset style="border: none; padding: 0;">
      <legend style="margin-bottom: 1rem;">Select your preferences:</legend>
      <div style="display: flex; flex-direction: column; gap: 0.5rem;">
        <label>
          <input type="checkbox" name="preferences" value="notifications" checked>
          Email notifications
        </label>
        <label>
          <input type="checkbox" name="preferences" value="newsletter">
          Weekly newsletter
        </label>
        <label>
          <input type="checkbox" name="preferences" value="updates">
          Product updates
        </label>
        <label>
          <input type="checkbox" name="preferences" value="promotions">
          Promotional offers
        </label>
      </div>
    </fieldset>
  `,
  description: "Group of related checkboxes"
}

export const FormExample = {
  name: "Form with Checkboxes",
  html: `
    <form style="display: flex; flex-direction: column; gap: 1.5rem; max-width: 48ch;">
      <div>
        <label for="username-cb" style="display: block; margin-bottom: 0.5rem;">Username</label>
        <input is-="input" box-="square" id="username-cb" placeholder="Enter username">
      </div>
      <fieldset style="border: none; padding: 0;">
        <legend style="margin-bottom: 0.5rem;">Account options:</legend>
        <div style="display: flex; flex-direction: column; gap: 0.5rem;">
          <label>
            <input type="checkbox" checked>
            Remember me
          </label>
          <label>
            <input type="checkbox">
            Subscribe to newsletter
          </label>
          <label>
            <input type="checkbox">
            Make profile public
          </label>
        </div>
      </fieldset>
      <label>
        <input type="checkbox" required>
        I agree to the <a href="#">terms of service</a>
      </label>
      <button is-="button" box-="square" type="submit">Create Account</button>
    </form>
  `,
  description: "Complete form with checkboxes"
}

export const TaskList = {
  name: "Task List Example",
  html: `
    <div style="display: flex; flex-direction: column; gap: 0.5rem;">
      <h3 style="margin-bottom: 0.5rem;">Today's Tasks</h3>
      <label style="text-decoration: line-through; color: var(--foreground2);">
        <input type="checkbox" checked>
        Set up development environment
      </label>
      <label style="text-decoration: line-through; color: var(--foreground2);">
        <input type="checkbox" checked>
        Install WebTUI components
      </label>
      <label>
        <input type="checkbox">
        Create component stories
      </label>
      <label>
        <input type="checkbox">
        Test all components
      </label>
      <label>
        <input type="checkbox" disabled>
        Deploy to production (blocked)
      </label>
    </div>
  `,
  description: "Checkboxes used for task tracking"
}