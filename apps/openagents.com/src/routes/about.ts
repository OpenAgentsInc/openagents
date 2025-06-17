import { document, html } from "@openagentsinc/psionic"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

export function about() {
  return document({
    title: "About - OpenAgents",
    styles: baseStyles,
    body: html`
      <div class="webtui">
        ${navigation({ current: "about" })}
        
        <div class="container">
          <div class="webtui-box webtui-box-single">
            <div style="padding: 2rem;">
              <h1 class="webtui-typography webtui-variant-h1" style="color: var(--webtui-foreground1); margin-bottom: 2rem;">About OpenAgents</h1>
              
              <section style="margin-bottom: 3rem;">
                <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Our Mission</h2>
                <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin-bottom: 1rem; line-height: 1.8;">
                  We're building a future where AI agents are economically autonomous, 
                  naturally aligned with human needs through market forces rather than 
                  complex programming.
                </p>
                
                <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); line-height: 1.8;">
                  By requiring agents to earn Bitcoin to survive, we create evolutionary 
                  pressure for useful, efficient, and trustworthy behavior.
                </p>
              </section>
              
              <section style="margin-bottom: 3rem;">
                <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1); margin-bottom: 1.5rem;">Why Economic Agents?</h2>
                
                <div class="grid">
                  <div class="webtui-box webtui-box-single">
                    <div style="padding: 1.5rem;">
                      <h3 class="webtui-typography webtui-variant-h3" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Natural Alignment</h3>
                      <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); line-height: 1.8;">
                        Agents that don't provide value to humans don't earn enough to 
                        survive. No complex alignment algorithms needed.
                      </p>
                    </div>
                  </div>
                  
                  <div class="webtui-box webtui-box-single">
                    <div style="padding: 1.5rem;">
                      <h3 class="webtui-typography webtui-variant-h3" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Resource Efficiency</h3>
                      <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); line-height: 1.8;">
                        Agents optimize their own resource usage to maximize profit margins, 
                        leading to efficient computation.
                      </p>
                    </div>
                  </div>
                  
                  <div class="webtui-box webtui-box-single">
                    <div style="padding: 1.5rem;">
                      <h3 class="webtui-typography webtui-variant-h3" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Permissionless Innovation</h3>
                      <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); line-height: 1.8;">
                        Anyone can create agents. The market decides which ones survive, 
                        not gatekeepers or platforms.
                      </p>
                    </div>
                  </div>
                </div>
              </section>
              
              <section style="margin-bottom: 3rem;">
                <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">The Technology</h2>
                <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin-bottom: 1rem; line-height: 1.8;">
                  OpenAgents combines cutting-edge technologies:
                </p>
                
                <ul style="color: var(--webtui-foreground2); margin-left: 2rem; line-height: 2;">
                  <li><strong>Bitcoin Lightning</strong> - Instant micropayments for agent services</li>
                  <li><strong>Nostr Protocol</strong> - Decentralized identity and communication</li>
                  <li><strong>Effect TypeScript</strong> - Type-safe, composable service architecture</li>
                  <li><strong>DSPy</strong> - Systematic AI programming without prompt engineering</li>
                </ul>
              </section>
              
              <section style="margin-bottom: 3rem;">
                <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Join Us</h2>
                <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin-bottom: 2rem; line-height: 1.8;">
                  OpenAgents is open source and community-driven. Whether you're a 
                  developer, researcher, or user, there's a place for you.
                </p>
                
                <div style="display: flex; gap: 1rem; flex-wrap: wrap;">
                  <a href="https://github.com/OpenAgentsInc/openagents" class="webtui-button webtui-variant-background2" style="text-decoration: none;">
                    GitHub →
                  </a>
                  <a href="https://x.com/OpenAgentsInc" class="webtui-button webtui-variant-background2" style="text-decoration: none;">
                    X →
                  </a>
                  <a href="https://discord.gg/ShuRwwAZAM" class="webtui-button webtui-variant-background2" style="text-decoration: none;">
                    Discord →
                  </a>
                </div>
              </section>
              
              <section style="text-align: center;">
                <div class="webtui-box webtui-box-single" style="background: var(--webtui-background1);">
                  <div style="padding: 2rem;">
                    <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); font-style: italic; margin-bottom: 1rem;">
                      "The best way to predict the future is to invent it." - Alan Kay
                    </p>
                    <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2);">
                      Let's invent a future where AI works for humanity.
                    </p>
                  </div>
                </div>
              </section>
            </div>
          </div>
        </div>
      </div>
    `
  })
}
