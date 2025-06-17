export const title = "WebTUI Switch"
export const component = "Switch"

export const Default = {
  name: "Default Switch",
  html: `<input type="checkbox" is-="switch">`,
  description: "Basic toggle switch"
}

export const WithLabel = {
  name: "Switch with Label",
  html: `
    <label>
      <input type="checkbox" is-="switch">
      Enable notifications
    </label>
  `,
  description: "Switch with associated label"
}

export const States = {
  name: "Switch States",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <label>
        <input type="checkbox" is-="switch">
        Off state
      </label>
      <label>
        <input type="checkbox" is-="switch" checked>
        On state
      </label>
      <label>
        <input type="checkbox" is-="switch" disabled>
        Disabled off
      </label>
      <label>
        <input type="checkbox" is-="switch" checked disabled>
        Disabled on
      </label>
    </div>
  `,
  description: "Different switch states"
}

export const Sizes = {
  name: "Switch Sizes",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <label>
        <input type="checkbox" is-="switch" size-="small">
        Small switch
      </label>
      <label>
        <input type="checkbox" is-="switch">
        Default switch
      </label>
    </div>
  `,
  description: "Switch size variations"
}

export const BarStyles = {
  name: "Switch Bar Styles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <label>
        <input type="checkbox" is-="switch">
        Default bar
      </label>
      <label>
        <input type="checkbox" is-="switch" bar-="thin">
        Thin bar
      </label>
      <label>
        <input type="checkbox" is-="switch" bar-="line">
        Line bar
      </label>
    </div>
  `,
  description: "Different switch track styles"
}

export const BoxStyles = {
  name: "Switch with Box Styles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <label>
        <input type="checkbox" is-="switch" box-="square">
        Square box switch
      </label>
      <label>
        <input type="checkbox" is-="switch" box-="round">
        Round box switch
      </label>
      <label>
        <input type="checkbox" is-="switch" box-="double">
        Double box switch
      </label>
    </div>
  `,
  description: "Switches with box borders"
}

export const SettingsExample = {
  name: "Settings Panel",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1.5rem; max-width: 48ch;">
      <h3>Notification Settings</h3>
      
      <div style="display: flex; flex-direction: column; gap: 1rem;">
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <span>Email notifications</span>
          <input type="checkbox" is-="switch" checked>
        </label>
        
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <span>Push notifications</span>
          <input type="checkbox" is-="switch">
        </label>
        
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <span>SMS alerts</span>
          <input type="checkbox" is-="switch">
        </label>
        
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <span>Marketing emails</span>
          <input type="checkbox" is-="switch" disabled>
        </label>
      </div>
      
      <hr style="border: none; border-top: 1px solid var(--background2);">
      
      <h3>Privacy Settings</h3>
      
      <div style="display: flex; flex-direction: column; gap: 1rem;">
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <span>Public profile</span>
          <input type="checkbox" is-="switch" checked>
        </label>
        
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <span>Show online status</span>
          <input type="checkbox" is-="switch" checked>
        </label>
        
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <span>Allow friend requests</span>
          <input type="checkbox" is-="switch">
        </label>
      </div>
    </div>
  `,
  description: "Switches in a settings panel layout"
}

export const FeatureToggles = {
  name: "Feature Toggles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <div style="background: var(--background1); padding: 1rem; border-radius: 4px;">
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <div>
            <div style="font-weight: bold;">Dark Mode</div>
            <div style="font-size: 0.9em; color: var(--foreground2);">Use dark theme across the application</div>
          </div>
          <input type="checkbox" is-="switch" checked>
        </label>
      </div>
      
      <div style="background: var(--background1); padding: 1rem; border-radius: 4px;">
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <div>
            <div style="font-weight: bold;">Auto-save</div>
            <div style="font-size: 0.9em; color: var(--foreground2);">Automatically save your work</div>
          </div>
          <input type="checkbox" is-="switch" checked>
        </label>
      </div>
      
      <div style="background: var(--background1); padding: 1rem; border-radius: 4px;">
        <label style="display: flex; justify-content: space-between; align-items: center;">
          <div>
            <div style="font-weight: bold;">Beta Features</div>
            <div style="font-size: 0.9em; color: var(--foreground2);">Enable experimental features</div>
          </div>
          <input type="checkbox" is-="switch">
        </label>
      </div>
    </div>
  `,
  description: "Feature toggles with descriptions"
}