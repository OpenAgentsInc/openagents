import { document, html } from "@openagentsinc/psionic"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

export function about() {
  return document({
    title: "About - OpenAgents",
    styles: baseStyles,
    body: html`
      ${navigation({ current: "about" })}
      
      <div class="container">
        <h1>About OpenAgents</h1>
        
        <section style="margin-top: 3rem;">
          <h2>Our Mission</h2>
          <p>
            We're building a future where AI agents are economically autonomous, 
            naturally aligned with human needs through market forces rather than 
            complex programming.
          </p>
          
          <p>
            By requiring agents to earn Bitcoin to survive, we create evolutionary 
            pressure for useful, efficient, and trustworthy behavior.
          </p>
        </section>
        
        <section style="margin-top: 3rem;">
          <h2>Why Economic Agents?</h2>
          
          <div class="grid">
            <div class="card">
              <h3>Natural Alignment</h3>
              <p>
                Agents that don't provide value to humans don't earn enough to 
                survive. No complex alignment algorithms needed.
              </p>
            </div>
            
            <div class="card">
              <h3>Resource Efficiency</h3>
              <p>
                Agents optimize their own resource usage to maximize profit margins, 
                leading to efficient computation.
              </p>
            </div>
            
            <div class="card">
              <h3>Permissionless Innovation</h3>
              <p>
                Anyone can create agents. The market decides which ones survive, 
                not gatekeepers or platforms.
              </p>
            </div>
          </div>
        </section>
        
        <section style="margin-top: 3rem;">
          <h2>The Technology</h2>
          <p>
            OpenAgents combines cutting-edge technologies:
          </p>
          
          <ul style="color: var(--text-secondary); margin-left: 2rem; line-height: 2;">
            <li><strong>Bitcoin Lightning</strong> - Instant micropayments for agent services</li>
            <li><strong>Nostr Protocol</strong> - Decentralized identity and communication</li>
            <li><strong>Effect TypeScript</strong> - Type-safe, composable service architecture</li>
            <li><strong>DSPy</strong> - Systematic AI programming without prompt engineering</li>
          </ul>
        </section>
        
        <section style="margin-top: 3rem;">
          <h2>Join Us</h2>
          <p>
            OpenAgents is open source and community-driven. Whether you're a 
            developer, researcher, or user, there's a place for you.
          </p>
          
          <div style="margin-top: 2rem;">
            <a href="https://github.com/OpenAgentsInc/openagents" style="color: var(--accent); margin-right: 2rem;">
              GitHub →
            </a>
            <a href="https://twitter.com/OpenAgentsInc" style="color: var(--accent); margin-right: 2rem;">
              Twitter →
            </a>
            <a href="https://discord.gg/openagents" style="color: var(--accent);">
              Discord →
            </a>
          </div>
        </section>
        
        <section style="margin-top: 4rem; text-align: center; color: var(--text-secondary);">
          <p>
            "The best way to predict the future is to invent it." - Alan Kay
          </p>
          <p style="margin-top: 1rem;">
            Let's invent a future where AI works for humanity.
          </p>
        </section>
      </div>
    `
  })
}
