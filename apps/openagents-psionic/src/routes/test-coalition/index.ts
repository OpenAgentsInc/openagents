/**
 * Coalition Formation Test Page
 * Demonstrates multi-agent coalition formation for complex projects
 */

import { html } from "@openagentsinc/psionic"
import type * as SDK from "@openagentsinc/sdk"
import { navigation } from "../../components/navigation.js"

export default () => {
  // Mock complex project requiring multiple skills
  const complexProject: SDK.Browser.ComplexProject = {
    id: `project_${Date.now()}`,
    requesterId: "client_001",
    title: "E-commerce Platform Development",
    description:
      "Build a complete e-commerce platform with AI-powered recommendations, real-time analytics, and payment processing. The platform should include user authentication, product catalog, shopping cart, checkout flow, and admin dashboard. Testing and documentation are required.",
    requirements: [
      {
        skill: "backend-development",
        priority: "required",
        estimatedTokens: 50000,
        estimatedDurationHours: 40
      },
      {
        skill: "frontend-development",
        priority: "required",
        estimatedTokens: 40000,
        estimatedDurationHours: 35
      },
      {
        skill: "ai-integration",
        priority: "required",
        estimatedTokens: 30000,
        estimatedDurationHours: 20
      },
      {
        skill: "testing",
        priority: "preferred",
        estimatedTokens: 20000,
        estimatedDurationHours: 15
      },
      {
        skill: "documentation",
        priority: "preferred",
        estimatedTokens: 15000,
        estimatedDurationHours: 10
      }
    ],
    totalBudgetSats: 250000,
    deadlineTimestamp: Date.now() + (7 * 24 * 60 * 60 * 1000), // 7 days
    minAgentsRequired: 3,
    maxAgentsAllowed: 5
  }

  return html`
    <!doctype html>
    <html lang="en" variant-="catppuccin">
      <head>
        <meta charset="utf-8" />
        <title>Coalition Formation Test - OpenAgents</title>
        <link rel="stylesheet" href="/css/main.css" />
        <link rel="stylesheet" href="/css/webtui.css" />
        <script src="/js/theme.js"></script>
      </head>
      <body>
        ${navigation({ current: "test" })}
        
        <main>
          <div class="container">
            <h1>Coalition Formation Test</h1>
            <p>Test multi-agent coalition formation for complex projects</p>
            
            <section box-="double">
              <h2>Complex Project Details</h2>
              <dl>
                <dt>Title:</dt>
                <dd>${complexProject.title}</dd>
                
                <dt>Description:</dt>
                <dd>${complexProject.description}</dd>
                
                <dt>Total Budget:</dt>
                <dd>${complexProject.totalBudgetSats.toLocaleString()} sats</dd>
                
                <dt>Deadline:</dt>
                <dd>${new Date(complexProject.deadlineTimestamp).toLocaleDateString()}</dd>
                
                <dt>Required Agents:</dt>
                <dd>${complexProject.minAgentsRequired} - ${complexProject.maxAgentsAllowed}</dd>
              </dl>
              
              <h3>Requirements</h3>
              <table>
                <thead>
                  <tr>
                    <th>Skill</th>
                    <th>Priority</th>
                    <th>Est. Tokens</th>
                    <th>Est. Hours</th>
                  </tr>
                </thead>
                <tbody>
                  ${
    complexProject.requirements.map((req) => `
                    <tr>
                      <td>${req.skill}</td>
                      <td><span is-="badge" variant-="${
      req.priority === "required" ? "foreground1" : "foreground2"
    }">${req.priority}</span></td>
                      <td>${req.estimatedTokens.toLocaleString()}</td>
                      <td>${req.estimatedDurationHours}h</td>
                    </tr>
                  `).join("")
  }
                </tbody>
              </table>
            </section>
            
            <section box-="single">
              <h2>Coalition Formation Process</h2>
              
              <div id="coalition-status">
                <p>Click the buttons below to simulate the coalition formation process.</p>
              </div>
              
              <div class="button-group">
                <button is-="button" variant-="foreground1" box-="double" onclick="analyzeRequirements()">
                  1. Analyze Requirements
                </button>
                
                <button is-="button" variant-="foreground1" box-="double" onclick="findAgents()" disabled>
                  2. Find Complementary Agents
                </button>
                
                <button is-="button" variant-="foreground1" box-="double" onclick="assessViability()" disabled>
                  3. Assess Coalition Viability
                </button>
                
                <button is-="button" variant-="foreground1" box-="double" onclick="proposeCoalition()" disabled>
                  4. Propose Coalition
                </button>
                
                <button is-="button" variant-="foreground1" box-="double" onclick="negotiateTerms()" disabled>
                  5. Negotiate Terms
                </button>
                
                <button is-="button" variant-="foreground1" box-="double" onclick="formalizeAgreement()" disabled>
                  6. Formalize Agreement
                </button>
              </div>
            </section>
            
            <section box-="single" id="results" style="display: none;">
              <h2>Results</h2>
              <pre id="results-content"></pre>
            </section>
          </div>
        </main>
        
        <script>
          // Store state for the demo
          let projectRequirements = null;
          let agentMatches = null;
          let viabilityScore = null;
          let coalitionProposal = null;
          let coalitionContract = null;
          
          async function updateStatus(message, type = 'info') {
            const statusDiv = document.getElementById('coalition-status');
            statusDiv.innerHTML = \`<p class="\${type}">\${message}</p>\`;
          }
          
          async function showResults(title, content) {
            const resultsSection = document.getElementById('results');
            const resultsContent = document.getElementById('results-content');
            resultsSection.style.display = 'block';
            resultsContent.textContent = title + '\\n\\n' + JSON.stringify(content, null, 2);
          }
          
          async function analyzeRequirements() {
            updateStatus('Analyzing project requirements...', 'loading');
            
            try {
              const response = await fetch('/api/coalition/analyze-requirements', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ project: ${JSON.stringify(complexProject)} })
              });
              
              const result = await response.json();
              
              if (result.success) {
                projectRequirements = result.requirements;
                updateStatus('Requirements analyzed successfully!', 'success');
                showResults('Project Requirements Analysis', projectRequirements);
                
                // Enable next button
                document.querySelector('button[onclick="findAgents()"]').disabled = false;
              } else {
                updateStatus('Failed to analyze requirements: ' + result.error, 'error');
              }
            } catch (error) {
              updateStatus('Error: ' + error.message, 'error');
            }
          }
          
          async function findAgents() {
            updateStatus('Finding complementary agents...', 'loading');
            
            try {
              const response = await fetch('/api/coalition/find-agents', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ requirements: projectRequirements })
              });
              
              const result = await response.json();
              
              if (result.success) {
                agentMatches = result.matches;
                updateStatus(\`Found \${agentMatches.length} matching agents!\`, 'success');
                showResults('Agent Matches', agentMatches);
                
                // Enable next button
                document.querySelector('button[onclick="assessViability()"]').disabled = false;
              } else {
                updateStatus('Failed to find agents: ' + result.error, 'error');
              }
            } catch (error) {
              updateStatus('Error: ' + error.message, 'error');
            }
          }
          
          async function assessViability() {
            updateStatus('Assessing coalition viability...', 'loading');
            
            // Select top agents for coalition
            const selectedAgents = agentMatches.slice(0, 5).map(match => match.agent);
            
            try {
              const response = await fetch('/api/coalition/assess-viability', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ 
                  agents: selectedAgents,
                  project: ${JSON.stringify(complexProject)}
                })
              });
              
              const result = await response.json();
              
              if (result.success) {
                viabilityScore = result.viability;
                updateStatus(\`Coalition viability: \${Math.round(viabilityScore.score * 100)}%\`, 'success');
                showResults('Viability Assessment', viabilityScore);
                
                // Enable next button if viable
                if (viabilityScore.score > 0.6) {
                  document.querySelector('button[onclick="proposeCoalition()"]').disabled = false;
                }
              } else {
                updateStatus('Failed to assess viability: ' + result.error, 'error');
              }
            } catch (error) {
              updateStatus('Error: ' + error.message, 'error');
            }
          }
          
          async function proposeCoalition() {
            updateStatus('Creating coalition proposal...', 'loading');
            
            const selectedAgents = agentMatches.slice(0, 5).map(match => match.agent);
            
            try {
              const response = await fetch('/api/coalition/propose', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ 
                  project: ${JSON.stringify(complexProject)},
                  agents: selectedAgents
                })
              });
              
              const result = await response.json();
              
              if (result.success) {
                coalitionProposal = result.proposal;
                updateStatus('Coalition proposal created!', 'success');
                showResults('Coalition Proposal', coalitionProposal);
                
                // Enable next button
                document.querySelector('button[onclick="negotiateTerms()"]').disabled = false;
              } else {
                updateStatus('Failed to create proposal: ' + result.error, 'error');
              }
            } catch (error) {
              updateStatus('Error: ' + error.message, 'error');
            }
          }
          
          async function negotiateTerms() {
            updateStatus('Negotiating coalition terms...', 'loading');
            
            try {
              const response = await fetch('/api/coalition/negotiate', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ proposal: coalitionProposal })
              });
              
              const result = await response.json();
              
              if (result.success) {
                coalitionContract = result.contract;
                updateStatus('Terms negotiated successfully!', 'success');
                showResults('Coalition Contract', coalitionContract);
                
                // Enable next button
                document.querySelector('button[onclick="formalizeAgreement()"]').disabled = false;
              } else {
                updateStatus('Failed to negotiate terms: ' + result.error, 'error');
              }
            } catch (error) {
              updateStatus('Error: ' + error.message, 'error');
            }
          }
          
          async function formalizeAgreement() {
            updateStatus('Formalizing coalition agreement...', 'loading');
            
            try {
              const response = await fetch('/api/coalition/formalize', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ contract: coalitionContract })
              });
              
              const result = await response.json();
              
              if (result.success) {
                const coalition = result.coalition;
                updateStatus('Coalition formed successfully! 🎉', 'success');
                showResults('Active Coalition', coalition);
                
                // Show link to project coordination
                updateStatus(\`Coalition formed! <a href="/test-coalition/project-coordination?coalitionId=\${coalition.coalitionId}">View Project Coordination →</a>\`, 'success');
              } else {
                updateStatus('Failed to formalize agreement: ' + result.error, 'error');
              }
            } catch (error) {
              updateStatus('Error: ' + error.message, 'error');
            }
          }
        </script>
      </body>
    </html>
  `
}
