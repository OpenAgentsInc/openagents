export const title = "WebTUI Textarea"
export const component = "Textarea"

export const Default = {
  name: "Default Textarea",
  html: `<textarea is-="textarea" placeholder="Enter multiple lines of text..."></textarea>`,
  description: "Basic textarea with default styling"
}

export const Sizes = {
  name: "Textarea Sizes",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <textarea is-="textarea" size-="small" placeholder="Small textarea"></textarea>
      <textarea is-="textarea" placeholder="Default textarea"></textarea>
      <textarea is-="textarea" size-="large" placeholder="Large textarea"></textarea>
    </div>
  `,
  description: "Textarea size variations"
}

export const BoxStyles = {
  name: "Textarea Box Styles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <textarea is-="textarea" box-="square" placeholder="Square box"></textarea>
      <textarea is-="textarea" box-="round" placeholder="Round box"></textarea>
      <textarea is-="textarea" box-="double" placeholder="Double box"></textarea>
    </div>
  `,
  description: "Different ASCII box border styles"
}

export const WithContent = {
  name: "Textarea with Content",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <textarea is-="textarea" rows="5">Line 1
Line 2
Line 3
Line 4
Line 5</textarea>
      <textarea is-="textarea" box-="square" rows="3">This is a textarea with some initial content.
It can contain multiple lines of text.
The resize handle allows vertical resizing.</textarea>
    </div>
  `,
  description: "Textareas with pre-filled content"
}

export const States = {
  name: "Textarea States",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <textarea is-="textarea" placeholder="Normal state"></textarea>
      <textarea is-="textarea" placeholder="Focused state" autofocus></textarea>
      <textarea is-="textarea" placeholder="Disabled state" disabled></textarea>
      <textarea is-="textarea" placeholder="Readonly state" readonly>Read-only content</textarea>
    </div>
  `,
  description: "Different textarea states"
}

export const FormExample = {
  name: "Form with Textarea",
  html: `
    <form style="display: flex; flex-direction: column; gap: 1.5rem; max-width: 48ch;">
      <div>
        <label for="subject" style="display: block; margin-bottom: 0.5rem;">Subject</label>
        <input is-="input" box-="square" id="subject" placeholder="Enter subject">
      </div>
      <div>
        <label for="message" style="display: block; margin-bottom: 0.5rem;">Message</label>
        <textarea is-="textarea" box-="square" id="message" rows="6" placeholder="Enter your message..."></textarea>
      </div>
      <div>
        <label for="notes" style="display: block; margin-bottom: 0.5rem;">Additional Notes</label>
        <textarea is-="textarea" box-="square" size-="small" id="notes" rows="3" placeholder="Optional notes..."></textarea>
      </div>
      <div style="display: flex; gap: 1rem;">
        <button is-="button" box-="square" type="submit">Send</button>
        <button is-="button" box-="square" variant-="background1" type="reset">Clear</button>
      </div>
    </form>
  `,
  description: "Complete form example with textareas"
}

export const CodeEditor = {
  name: "Code Editor Example",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <div style="display: flex; justify-content: space-between; align-items: center;">
        <span>config.json</span>
        <button is-="button" size-="small" box-="square">Save</button>
      </div>
      <textarea is-="textarea" box-="double" rows="10" style="font-family: 'Berkeley Mono', ui-monospace, 'Cascadia Code', 'Source Code Pro', Menlo, Consolas, 'DejaVu Sans Mono', monospace;">{
  "name": "webtui-demo",
  "version": "1.0.0",
  "description": "WebTUI component demo",
  "theme": {
    "primary": "foreground0",
    "background": "background1"
  }
}</textarea>
    </div>
  `,
  description: "Textarea used as a simple code editor"
}