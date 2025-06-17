export const title = "WebTUI Radio"
export const component = "Radio"

export const Default = {
  name: "Default Radio",
  html: `<input type="radio" name="single">`,
  description: "Basic radio button with WebTUI styling"
}

export const RadioGroup = {
  name: "Radio Group",
  html: `
    <fieldset style="border: none; padding: 0;">
      <legend style="margin-bottom: 1rem;">Select your preference:</legend>
      <div style="display: flex; flex-direction: column; gap: 0.5rem;">
        <label>
          <input type="radio" name="preference" value="option1" checked>
          Option 1
        </label>
        <label>
          <input type="radio" name="preference" value="option2">
          Option 2
        </label>
        <label>
          <input type="radio" name="preference" value="option3">
          Option 3
        </label>
      </div>
    </fieldset>
  `,
  description: "Group of radio buttons (single selection)"
}

export const States = {
  name: "Radio States",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <label>
        <input type="radio" name="states1">
        Unchecked
      </label>
      <label>
        <input type="radio" name="states2" checked>
        Checked
      </label>
      <label>
        <input type="radio" name="states3" disabled>
        Disabled unchecked
      </label>
      <label>
        <input type="radio" name="states4" checked disabled>
        Disabled checked
      </label>
    </div>
  `,
  description: "Different radio button states"
}

export const BoxStyles = {
  name: "Radio with Box Styles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <label>
        <input type="radio" name="boxstyle" box-="square">
        Square box radio
      </label>
      <label>
        <input type="radio" name="boxstyle" box-="round">
        Round box radio
      </label>
      <label>
        <input type="radio" name="boxstyle" box-="double">
        Double box radio
      </label>
    </div>
  `,
  description: "Radio buttons with box border styles"
}

export const InlineRadios = {
  name: "Inline Radio Layout",
  html: `
    <div>
      <p style="margin-bottom: 0.5rem;">Size:</p>
      <div style="display: flex; gap: 1.5rem;">
        <label>
          <input type="radio" name="size" value="small">
          Small
        </label>
        <label>
          <input type="radio" name="size" value="medium" checked>
          Medium
        </label>
        <label>
          <input type="radio" name="size" value="large">
          Large
        </label>
      </div>
    </div>
  `,
  description: "Horizontal radio button layout"
}

export const FormExample = {
  name: "Form with Radio Groups",
  html: `
    <form style="display: flex; flex-direction: column; gap: 1.5rem; max-width: 48ch;">
      <fieldset style="border: none; padding: 0;">
        <legend style="margin-bottom: 0.5rem;">Subscription Plan:</legend>
        <div style="display: flex; flex-direction: column; gap: 0.5rem;">
          <label>
            <input type="radio" name="plan" value="free" checked>
            Free - $0/month
          </label>
          <label>
            <input type="radio" name="plan" value="basic">
            Basic - $9/month
          </label>
          <label>
            <input type="radio" name="plan" value="pro">
            Pro - $19/month
          </label>
          <label>
            <input type="radio" name="plan" value="enterprise">
            Enterprise - Contact us
          </label>
        </div>
      </fieldset>
      
      <fieldset style="border: none; padding: 0;">
        <legend style="margin-bottom: 0.5rem;">Billing Cycle:</legend>
        <div style="display: flex; gap: 1.5rem;">
          <label>
            <input type="radio" name="billing" value="monthly" checked>
            Monthly
          </label>
          <label>
            <input type="radio" name="billing" value="yearly">
            Yearly (save 20%)
          </label>
        </div>
      </fieldset>
      
      <button is-="button" box-="square" type="submit">Continue</button>
    </form>
  `,
  description: "Complete form with multiple radio groups"
}

export const SurveyExample = {
  name: "Survey Example",
  html: `
    <div style="display: flex; flex-direction: column; gap: 2rem; max-width: 48ch;">
      <div>
        <p style="margin-bottom: 0.5rem;">How satisfied are you with our service?</p>
        <div style="display: flex; flex-direction: column; gap: 0.5rem;">
          <label>
            <input type="radio" name="satisfaction" value="5">
            Very satisfied
          </label>
          <label>
            <input type="radio" name="satisfaction" value="4">
            Satisfied
          </label>
          <label>
            <input type="radio" name="satisfaction" value="3">
            Neutral
          </label>
          <label>
            <input type="radio" name="satisfaction" value="2">
            Dissatisfied
          </label>
          <label>
            <input type="radio" name="satisfaction" value="1">
            Very dissatisfied
          </label>
        </div>
      </div>
      
      <div>
        <p style="margin-bottom: 0.5rem;">Would you recommend us to a friend?</p>
        <div style="display: flex; gap: 1.5rem;">
          <label>
            <input type="radio" name="recommend" value="yes">
            Yes
          </label>
          <label>
            <input type="radio" name="recommend" value="no">
            No
          </label>
          <label>
            <input type="radio" name="recommend" value="maybe">
            Maybe
          </label>
        </div>
      </div>
    </div>
  `,
  description: "Radio buttons in a survey context"
}