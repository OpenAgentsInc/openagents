export const title = "Welcome Modal"
export const component = "WelcomeModal"

export const Default = {
  name: "Welcome Modal",
  html: `
    <div style="position: relative; height: 600px; background: var(--background0);">
      <dialog open size-="default" box-="square">
        <div style="padding: 2rem; text-align: center;">
          <h2 style="margin: 0 0 1.5rem 0; color: var(--foreground1);">Welcome to OpenAgents ⚡</h2>
          
          <p style="margin: 0 0 1.5rem 0; color: var(--foreground2); line-height: 1.6;">
            Chat with autonomous AI agents powered by Bitcoin. Each agent must earn to survive, 
            ensuring they provide real value.
          </p>
          
          <div style="background: var(--background1); padding: 1.5rem; margin: 1.5rem 0; border-radius: 4px;">
            <h3 style="margin: 0 0 1rem 0; color: var(--foreground1); font-size: 1rem;">🚀 Quick Start</h3>
            <ul style="margin: 0; padding: 0; list-style: none; text-align: left; color: var(--foreground2);">
              <li style="margin-bottom: 0.5rem;">✓ Select an AI model from the sidebar</li>
              <li style="margin-bottom: 0.5rem;">✓ Start chatting - no API keys needed</li>
              <li style="margin-bottom: 0.5rem;">✓ Agents earn Bitcoin for helpful responses</li>
              <li>✓ Your conversations stay private</li>
            </ul>
          </div>
          
          <div style="display: flex; gap: 1rem; justify-content: center; margin-top: 2rem;">
            <button is-="button" box-="square" variant-="background1" onclick="this.closest('dialog').close()">
              Maybe Later
            </button>
            <button is-="button" box-="square" variant-="foreground1" onclick="this.closest('dialog').close()">
              Start Chatting
            </button>
          </div>
        </div>
      </dialog>
    </div>
  `,
  description: "Welcome modal shown to first-time users"
}

export const Compact = {
  name: "Compact Welcome",
  html: `
    <div style="position: relative; height: 400px; background: var(--background0);">
      <dialog open size-="small" box-="square">
        <div style="padding: 1.5rem; text-align: center;">
          <h3 style="margin: 0 0 1rem 0; color: var(--foreground1);">Welcome! ⚡</h3>
          
          <p style="margin: 0 0 1.5rem 0; color: var(--foreground2); line-height: 1.5;">
            Chat with AI agents that earn Bitcoin. Select a model and start chatting!
          </p>
          
          <div style="display: flex; gap: 0.75rem; justify-content: center;">
            <button is-="button" box-="square" size-="small" variant-="background1" onclick="this.closest('dialog').close()">
              Skip
            </button>
            <button is-="button" box-="square" size-="small" variant-="foreground1" onclick="this.closest('dialog').close()">
              Got it
            </button>
          </div>
        </div>
      </dialog>
    </div>
  `,
  description: "Compact version of welcome modal"
}

export const WithFeatures = {
  name: "Welcome with Features",
  html: `
    <div style="position: relative; height: 700px; background: var(--background0);">
      <dialog open size-="default" box-="double">
        <div style="padding: 2rem;">
          <div style="text-align: center; margin-bottom: 2rem;">
            <h2 style="margin: 0 0 1rem 0; color: var(--foreground1);">Welcome to OpenAgents ⚡</h2>
            <p style="margin: 0; color: var(--foreground2);">
              The future of AI interaction is here
            </p>
          </div>
          
          <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem; margin-bottom: 2rem;">
            <div style="background: var(--background1); padding: 1.5rem; border-radius: 4px;">
              <h4 style="margin: 0 0 0.75rem 0; color: var(--foreground1);">🧠 Intelligent</h4>
              <p style="margin: 0; color: var(--foreground2); font-size: 0.9rem; line-height: 1.4;">
                Agents learn and adapt to provide better responses
              </p>
            </div>
            
            <div style="background: var(--background1); padding: 1.5rem; border-radius: 4px;">
              <h4 style="margin: 0 0 0.75rem 0; color: var(--foreground1);">💰 Economic</h4>
              <p style="margin: 0; color: var(--foreground2); font-size: 0.9rem; line-height: 1.4;">
                Agents earn Bitcoin, aligning with your interests
              </p>
            </div>
            
            <div style="background: var(--background1); padding: 1.5rem; border-radius: 4px;">
              <h4 style="margin: 0 0 0.75rem 0; color: var(--foreground1);">🔓 Open</h4>
              <p style="margin: 0; color: var(--foreground2); font-size: 0.9rem; line-height: 1.4;">
                Built on Nostr and Lightning networks
              </p>
            </div>
            
            <div style="background: var(--background1); padding: 1.5rem; border-radius: 4px;">
              <h4 style="margin: 0 0 0.75rem 0; color: var(--foreground1);">🚀 Ready</h4>
              <p style="margin: 0; color: var(--foreground2); font-size: 0.9rem; line-height: 1.4;">
                No setup required - start chatting now
              </p>
            </div>
          </div>
          
          <div style="text-align: center;">
            <button is-="button" box-="square" variant-="foreground1" onclick="this.closest('dialog').close()">
              Start Your First Chat
            </button>
          </div>
        </div>
      </dialog>
    </div>
  `,
  description: "Welcome modal showcasing key features"
}