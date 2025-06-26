export const title = "WebTUI Dialog"
export const component = "Dialog"

export const Default = {
  name: "Default Dialog",
  html: `
    <div style="position: relative; height: 200px; background: var(--background1);">
      <dialog open>
        <h3>Default Dialog</h3>
        <p>This is a basic dialog with default positioning.</p>
        <button is-="button" box-="square">Close</button>
      </dialog>
    </div>
  `,
  description: "Basic dialog with default center positioning"
}

export const Sizes = {
  name: "Dialog Sizes",
  html: `
    <div style="display: flex; flex-direction: column; gap: 2rem;">
      <div style="position: relative; height: 200px; background: var(--background1);">
        <dialog open size-="small">
          <h4>Small Dialog</h4>
          <p>Compact dialog size.</p>
        </dialog>
      </div>
      
      <div style="position: relative; height: 300px; background: var(--background1);">
        <dialog open size-="default">
          <h4>Default Dialog</h4>
          <p>Standard dialog size with more content space.</p>
          <p>Additional paragraph to show content.</p>
        </dialog>
      </div>
    </div>
  `,
  description: "Different dialog sizes"
}

export const Positioning = {
  name: "Dialog Positioning",
  html: `
    <div style="position: relative; height: 400px; background: var(--background1);">
      <dialog open position-="start-start" style="--dialog-offset-x: 1rem; --dialog-offset-y: 1rem;">
        <p>Top-Left</p>
      </dialog>
      
      <dialog open position-="center-start" style="--dialog-offset-y: 1rem;">
        <p>Top-Center</p>
      </dialog>
      
      <dialog open position-="end-start" style="--dialog-offset-x: 1rem; --dialog-offset-y: 1rem;">
        <p>Top-Right</p>
      </dialog>
      
      <dialog open position-="start-center" style="--dialog-offset-x: 1rem;">
        <p>Middle-Left</p>
      </dialog>
      
      <dialog open position-="center-center">
        <p>Center</p>
      </dialog>
      
      <dialog open position-="end-center" style="--dialog-offset-x: 1rem;">
        <p>Middle-Right</p>
      </dialog>
      
      <dialog open position-="start-end" style="--dialog-offset-x: 1rem; --dialog-offset-y: 1rem;">
        <p>Bottom-Left</p>
      </dialog>
      
      <dialog open position-="center-end" style="--dialog-offset-y: 1rem;">
        <p>Bottom-Center</p>
      </dialog>
      
      <dialog open position-="end-end" style="--dialog-offset-x: 1rem; --dialog-offset-y: 1rem;">
        <p>Bottom-Right</p>
      </dialog>
    </div>
  `,
  description: "All dialog positioning options"
}

export const BoxStyles = {
  name: "Dialog with Box Styles",
  html: `
    <div style="display: flex; gap: 2rem;">
      <div style="position: relative; height: 200px; background: var(--background1); flex: 1;">
        <dialog open box-="square">
          <h4>Square Box</h4>
          <p>Dialog with square borders.</p>
        </dialog>
      </div>
      
      <div style="position: relative; height: 200px; background: var(--background1); flex: 1;">
        <dialog open box-="round">
          <h4>Round Box</h4>
          <p>Dialog with round borders.</p>
        </dialog>
      </div>
      
      <div style="position: relative; height: 200px; background: var(--background1); flex: 1;">
        <dialog open box-="double">
          <h4>Double Box</h4>
          <p>Dialog with double borders.</p>
        </dialog>
      </div>
    </div>
  `,
  description: "Dialogs with different box border styles"
}

export const ContainerModes = {
  name: "Container Modes",
  html: `
    <div style="display: flex; flex-direction: column; gap: 2rem;">
      <div style="position: relative; height: 200px; background: var(--background1);">
        <dialog open container-="auto-auto">
          <h4>Auto Width & Height</h4>
          <p>Dialog sizes to content.</p>
        </dialog>
      </div>
      
      <div style="position: relative; height: 200px; background: var(--background1);">
        <dialog open container-="fill-auto" style="--dialog-offset-x: 2rem;">
          <h4>Fill Width, Auto Height</h4>
          <p>Dialog fills available width with margin.</p>
        </dialog>
      </div>
      
      <div style="position: relative; height: 300px; background: var(--background1);">
        <dialog open container-="auto-fill" style="--dialog-offset-y: 2rem;">
          <h4>Auto Width, Fill Height</h4>
          <p>Dialog fills available height with margin.</p>
          <p>More content...</p>
          <p>And more...</p>
        </dialog>
      </div>
    </div>
  `,
  description: "Different container sizing modes"
}

export const ComplexDialog = {
  name: "Complex Dialog Example",
  html: `
    <div style="position: relative; height: 400px; background: var(--background1);">
      <dialog open size-="default" box-="double">
        <div style="padding: 1rem;">
          <h3 style="margin-bottom: 1rem;">Confirm Action</h3>
          
          <p style="margin-bottom: 1.5rem;">Are you sure you want to delete this item? This action cannot be undone.</p>
          
          <div style="background: var(--background1); padding: 1rem; margin-bottom: 1.5rem;">
            <strong>Item Details:</strong>
            <pre is-="pre" size-="small">Name: Important Document
Size: 2.4 MB
Created: 2024-12-01
Modified: 2024-12-15</pre>
          </div>
          
          <div style="display: flex; gap: 1rem; justify-content: flex-end;">
            <button is-="button" box-="square" variant-="background1">Cancel</button>
            <button is-="button" box-="square">Delete</button>
          </div>
        </div>
      </dialog>
    </div>
  `,
  description: "Complex dialog with form elements"
}

export const NotificationDialog = {
  name: "Notification Style Dialog",
  html: `
    <div style="position: relative; height: 300px; background: var(--background1);">
      <dialog open position-="end-start" size-="small" box-="square" style="--dialog-offset-x: 1rem; --dialog-offset-y: 1rem;">
        <div style="padding: 1rem;">
          <div style="display: flex; justify-content: space-between; align-items: start; margin-bottom: 0.5rem;">
            <strong>New Message</strong>
            <button is-="button" size-="small">[X]</button>
          </div>
          <p style="color: var(--foreground2);">You have received a new message from John Doe.</p>
          <div style="margin-top: 1rem;">
            <button is-="button" size-="small" box-="square">View</button>
          </div>
        </div>
      </dialog>
    </div>
  `,
  description: "Notification-style dialog positioned at top-right"
}

export const FormDialog = {
  name: "Form Dialog",
  html: `
    <div style="position: relative; height: 500px; background: var(--background1);">
      <dialog open size-="default" box-="square">
        <form style="padding: 1.5rem;">
          <h3 style="margin-bottom: 1.5rem;">Create New Item</h3>
          
          <div style="display: flex; flex-direction: column; gap: 1rem; margin-bottom: 1.5rem;">
            <div>
              <label for="item-name" style="display: block; margin-bottom: 0.5rem;">Name</label>
              <input is-="input" id="item-name" placeholder="Enter item name" style="width: 100%;">
            </div>
            
            <div>
              <label for="item-desc" style="display: block; margin-bottom: 0.5rem;">Description</label>
              <textarea is-="textarea" id="item-desc" rows="3" placeholder="Enter description" style="width: 100%;"></textarea>
            </div>
            
            <div>
              <label style="display: block; margin-bottom: 0.5rem;">Type</label>
              <div style="display: flex; flex-direction: column; gap: 0.5rem;">
                <label>
                  <input type="radio" name="type" value="document" checked>
                  Document
                </label>
                <label>
                  <input type="radio" name="type" value="image">
                  Image
                </label>
                <label>
                  <input type="radio" name="type" value="video">
                  Video
                </label>
              </div>
            </div>
          </div>
          
          <div style="display: flex; gap: 1rem; justify-content: flex-end;">
            <button is-="button" box-="square" variant-="background1" type="button">Cancel</button>
            <button is-="button" box-="square" type="submit">Create</button>
          </div>
        </form>
      </dialog>
    </div>
  `,
  description: "Dialog containing a complete form"
}