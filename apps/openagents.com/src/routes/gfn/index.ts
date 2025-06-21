import { document, html } from "@openagentsinc/psionic"
import { sharedHeader } from "../../components/shared-header.js"
import { baseStyles } from "../../styles.js"
import type { GFNParameters } from "./gfn-calculator.js"
import {
  ANTHROPIC_PARAMS,
  calculateGFN,
  DEFAULT_PARAMS,
  formatNumber,
  formatValue,
  getInsights,
  OPENAGENTS_PROJECTIONS,
  OPENAI_PARAMS
} from "./gfn-calculator.js"
import {
  createDominantEffectIndicator,
  createNetworkEffectChart,
  createProjectionChart,
  createValueBreakdown
} from "./gfn-visualizations.js"

export default function gfn() {
  // Use default parameters initially
  const params: GFNParameters = { ...DEFAULT_PARAMS }

  // Calculate results
  const results = calculateGFN(params)
  const insights = getInsights(params, results)

  // Calculate projections for OpenAgents
  const projections = Object.entries(OPENAGENTS_PROJECTIONS).map(([key, proj]) => ({
    label: key === "current" ?
      "Now" :
      key === "sixMonths" ?
      "6mo" :
      key === "oneYear" ?
      "1yr" :
      key === "twoYears"
      ? "2yr"
      : "5yr",
    n: proj.n,
    value: calculateGFN(proj).totalValue
  }))

  return document({
    title: "GFN Interactive Formula - OpenAgents",
    styles: baseStyles + `
      .gfn-container {
        max-width: 1400px;
        margin: 0 auto;
        padding: 2rem;
      }
      
      .gfn-header {
        text-align: center;
        margin-bottom: 3rem;
      }
      
      .gfn-main {
        display: grid;
        grid-template-columns: 350px 1fr;
        gap: 2rem;
      }
      
      @media (max-width: 768px) {
        .gfn-main {
          grid-template-columns: 1fr;
        }
      }
      
      .control-panel {
        background: var(--background1);
        padding: 1.5rem;
        border-radius: 0.5rem;
        max-height: calc(100vh - 200px);
        overflow-y: auto;
      }
      
      .slider-group {
        margin-bottom: 1.5rem;
      }
      
      .slider-label {
        display: flex;
        justify-content: space-between;
        margin-bottom: 0.5rem;
        font-size: 0.9em;
      }
      
      .slider {
        width: 100%;
        margin-bottom: 0.25rem;
      }
      
      .preset-buttons {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 2rem;
        flex-wrap: wrap;
      }
      
      .total-value {
        font-size: 4rem;
        font-weight: bold;
        color: var(--foreground0);
        text-align: center;
        margin-bottom: 2rem;
        text-shadow: 0 0 20px var(--foreground0);
      }
      
      .visualization-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 2rem;
        margin-bottom: 2rem;
      }
      
      @media (max-width: 1200px) {
        .visualization-grid {
          grid-template-columns: 1fr;
        }
      }
      
      .visualization-box {
        background: var(--background1);
        padding: 1.5rem;
        border-radius: 0.5rem;
      }
      
      .insights-panel {
        background: var(--background2);
        padding: 1.5rem;
        border-radius: 0.5rem;
        margin-bottom: 2rem;
      }
      
      .insight-item {
        padding: 0.5rem 0;
        border-bottom: 1px solid var(--foreground2);
      }
      
      .insight-item:last-child {
        border-bottom: none;
      }
      
      .formula-display {
        background: var(--background0);
        padding: 1rem;
        border-radius: 0.5rem;
        font-family: monospace;
        overflow-x: auto;
        margin-bottom: 2rem;
      }
      
      .help-text {
        font-size: 0.8em;
        color: var(--foreground2);
        margin-top: 0.25rem;
      }
      
      input[type="range"] {
        -webkit-appearance: none;
        appearance: none;
        background: transparent;
        cursor: pointer;
        width: 100%;
      }
      
      input[type="range"]::-webkit-slider-track {
        background: var(--background2);
        height: 6px;
        border-radius: 3px;
      }
      
      input[type="range"]::-webkit-slider-thumb {
        -webkit-appearance: none;
        appearance: none;
        background: var(--foreground0);
        height: 18px;
        width: 18px;
        border-radius: 50%;
        margin-top: -6px;
      }
      
      input[type="range"]::-moz-range-track {
        background: var(--background2);
        height: 6px;
        border-radius: 3px;
      }
      
      input[type="range"]::-moz-range-thumb {
        background: var(--foreground0);
        height: 18px;
        width: 18px;
        border-radius: 50%;
        border: none;
      }
      
      .section-title {
        font-size: 0.8em;
        text-transform: uppercase;
        color: var(--foreground1);
        margin-bottom: 1rem;
        letter-spacing: 0.1em;
      }
    `,
    body: html`
      <div class="fixed-layout">
        ${sharedHeader({ current: "gfn" })}
        
        <div class="gfn-container">
          <div class="gfn-header">
            <h1>Global Freedom Network Value Calculator</h1>
            <p style="color: var(--foreground1); margin-bottom: 1rem;">
              Explore how network effects combine to create exponential value
            </p>
            <div class="formula-display">
              V<sub>total</sub> = [α₁(k₁ × n) + α₂(k₂ × n²) + α₃(k₃ × 2ⁿ × C)] × Q × M × (1 + D)
            </div>
          </div>
          
          <div class="gfn-main">
            <div class="control-panel">
              <h2 style="margin-bottom: 1.5rem;">Parameters</h2>
              
              <div class="preset-buttons">
                <button is-="button" variant-="foreground1" onclick="setPreset('openai')">OpenAI</button>
                <button is-="button" variant-="foreground1" onclick="setPreset('anthropic')">Anthropic</button>
                <button is-="button" variant-="foreground0" onclick="setPreset('openagents_current')">OpenAgents Now</button>
                <button is-="button" variant-="foreground0" onclick="setPreset('openagents_6m')">OA 6mo</button>
                <button is-="button" variant-="foreground0" onclick="setPreset('openagents_1y')">OA 1yr</button>
                <button is-="button" variant-="foreground0" onclick="setPreset('openagents_2y')">OA 2yr</button>
                <button is-="button" variant-="foreground0" onclick="setPreset('openagents_5y')">OA 5yr</button>
              </div>
              
              <div class="section-title">Core Variables</div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>Active Participants (n)</span>
                  <span id="n_value">${formatNumber(params.n)}</span>
                </div>
                <input type="range" id="n" class="slider" 
                       min="10" max="50000000" step="1"
                       value="${params.n}" onchange="updateGFN()">
                <div class="help-text">Number of active network participants</div>
              </div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>Clustering Coefficient (C)</span>
                  <span id="C_value">${params.C.toFixed(2)}</span>
                </div>
                <input type="range" id="C" class="slider"
                       min="0.01" max="1" step="0.01"
                       value="${params.C}" onchange="updateGFN()">
                <div class="help-text">Group formation density (0-1)</div>
              </div>
              
              <div class="section-title">Network Type Coefficients (α)</div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>α₁ (Broadcast/Sarnoff)</span>
                  <span id="alpha1_value">${params.alpha1.toFixed(2)}</span>
                </div>
                <input type="range" id="alpha1" class="slider"
                       min="0.05" max="0.25" step="0.01"
                       value="${params.alpha1}" onchange="updateGFN()">
              </div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>α₂ (P2P/Metcalfe)</span>
                  <span id="alpha2_value">${params.alpha2.toFixed(2)}</span>
                </div>
                <input type="range" id="alpha2" class="slider"
                       min="0.25" max="0.45" step="0.01"
                       value="${params.alpha2}" onchange="updateGFN()">
              </div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>α₃ (Group/Reed)</span>
                  <span id="alpha3_value">${params.alpha3.toFixed(2)}</span>
                </div>
                <input type="range" id="alpha3" class="slider"
                       min="0.30" max="0.80" step="0.01"
                       value="${params.alpha3}" onchange="updateGFN()">
              </div>
              
              <div class="section-title">Value Per Connection (k)</div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>k₁ (Broadcast Value)</span>
                  <span id="k1_value">$${params.k1.toFixed(4)}</span>
                </div>
                <input type="range" id="k1" class="slider"
                       min="0.001" max="0.01" step="0.0001"
                       value="${params.k1}" onchange="updateGFN()">
              </div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>k₂ (P2P Value)</span>
                  <span id="k2_value">$${params.k2.toFixed(4)}</span>
                </div>
                <input type="range" id="k2" class="slider"
                       min="0.0005" max="0.003" step="0.00005"
                       value="${params.k2}" onchange="updateGFN()">
              </div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>k₃ (Group Value)</span>
                  <span id="k3_value">$${params.k3.toFixed(4)}</span>
                </div>
                <input type="range" id="k3" class="slider"
                       min="0.00005" max="0.001" step="0.00005"
                       value="${params.k3}" onchange="updateGFN()">
              </div>
              
              <div class="section-title">Multipliers</div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>Quality Factor (Q)</span>
                  <span id="Q_value">${params.Q.toFixed(1)}</span>
                </div>
                <input type="range" id="Q" class="slider"
                       min="0.5" max="3.0" step="0.1"
                       value="${params.Q}" onchange="updateGFN()">
                <div class="help-text">Engagement and output quality</div>
              </div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>Platform Multiplier (M)</span>
                  <span id="M_value">${params.M.toFixed(1)}</span>
                </div>
                <input type="range" id="M" class="slider"
                       min="1.0" max="4.0" step="0.1"
                       value="${params.M}" onchange="updateGFN()">
                <div class="help-text">Multi-sided platform effects</div>
              </div>
              
              <div class="slider-group">
                <div class="slider-label">
                  <span>Data Network Effect (D)</span>
                  <span id="D_value">${params.D.toFixed(1)}</span>
                </div>
                <input type="range" id="D" class="slider"
                       min="0.2" max="0.9" step="0.05"
                       value="${params.D}" onchange="updateGFN()">
                <div class="help-text">Value from data accumulation</div>
              </div>
            </div>
            
            <div class="visualizations">
              <div class="total-value">
                ${formatValue(results.totalValue)}
              </div>
              
              <div class="insights-panel">
                <h3 style="margin-bottom: 1rem;">Key Insights</h3>
                ${
      insights.map((insight) => `
                  <div class="insight-item">${insight}</div>
                `).join("")
    }
              </div>
              
              <div class="visualization-grid">
                <div class="visualization-box">
                  ${createDominantEffectIndicator(results, params.n)}
                </div>
                
                <div class="visualization-box">
                  ${createNetworkEffectChart(results)}
                </div>
                
                <div class="visualization-box">
                  ${createValueBreakdown(results, params)}
                </div>
                
                <div class="visualization-box">
                  ${createProjectionChart(projections)}
                </div>
              </div>
              
              <div style="background: var(--background1); padding: 1.5rem; border-radius: 0.5rem;">
                <h3 style="margin-bottom: 1rem;">Understanding the Formula</h3>
                <p style="margin-bottom: 1rem;">
                  The GFN formula captures how different types of network effects combine to create value:
                </p>
                <ul style="margin-left: 1.5rem;">
                  <li style="margin-bottom: 0.5rem;">
                    <strong>Sarnoff's Law (n):</strong> Linear growth from broadcast/content value
                  </li>
                  <li style="margin-bottom: 0.5rem;">
                    <strong>Metcalfe's Law (n²):</strong> Quadratic growth from P2P connections
                  </li>
                  <li style="margin-bottom: 0.5rem;">
                    <strong>Reed's Law (2ⁿ):</strong> Exponential growth from group formation
                  </li>
                </ul>
                <p style="margin-top: 1rem;">
                  AI agents unlock Reed's Law by transcending human cognitive limits, 
                  enabling unlimited parallel group participation and near-perfect clustering.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
      
      <script>
        // GFN calculation functions
        function calculateGFN(params) {
          const { n, C, alpha1, alpha2, alpha3, k1, k2, k3, Q, M, D } = params;
          
          // Calculate individual components
          const sarnoffComponent = k1 * n;
          const metcalfeComponent = k2 * n * n;
          
          // Reed's Law with safety check for exponential growth
          let reedComponent = 0;
          if (n < 50) {
            // For small n, calculate directly
            reedComponent = k3 * Math.pow(2, n) * C;
          } else {
            // For large n, use approximation to avoid overflow
            const logValue = n * Math.log(2) + Math.log(k3) + Math.log(Math.max(C, 0.0001));
            if (logValue < 100) { // e^100 is still manageable
              reedComponent = Math.exp(logValue);
            } else {
              // For extremely large values, cap at a reasonable maximum
              reedComponent = Number.MAX_SAFE_INTEGER / 1000;
            }
          }
          
          // Apply network type coefficients
          const sarnoffValue = alpha1 * sarnoffComponent;
          const metcalfeValue = alpha2 * metcalfeComponent;
          const reedValue = alpha3 * reedComponent;
          
          // Calculate base value
          const baseValue = sarnoffValue + metcalfeValue + reedValue;
          
          // Apply multipliers
          const totalValue = baseValue * Q * M * (1 + D);
          
          return totalValue;
        }
        
        function formatValue(key, value) {
          if (key === 'n') return value.toLocaleString();
          if (key === 'C' || key.startsWith('alpha')) return value.toFixed(2);
          if (key.startsWith('k')) return '$' + value.toFixed(4);
          if (key === 'Q' || key === 'M' || key === 'D') return value.toFixed(1);
          return value;
        }
        
        function formatDollarValue(value) {
          if (value >= 1e12) return '$' + (value / 1e12).toFixed(2) + 'T';
          if (value >= 1e9) return '$' + (value / 1e9).toFixed(2) + 'B';
          if (value >= 1e6) return '$' + (value / 1e6).toFixed(2) + 'M';
          if (value >= 1e3) return '$' + (value / 1e3).toFixed(2) + 'K';
          return '$' + value.toFixed(2);
        }
        
        function updateGFN() {
          const params = {
            n: parseInt(document.getElementById('n').value),
            C: parseFloat(document.getElementById('C').value),
            alpha1: parseFloat(document.getElementById('alpha1').value),
            alpha2: parseFloat(document.getElementById('alpha2').value),
            alpha3: parseFloat(document.getElementById('alpha3').value),
            k1: parseFloat(document.getElementById('k1').value),
            k2: parseFloat(document.getElementById('k2').value),
            k3: parseFloat(document.getElementById('k3').value),
            Q: parseFloat(document.getElementById('Q').value),
            M: parseFloat(document.getElementById('M').value),
            D: parseFloat(document.getElementById('D').value)
          };
          
          // Calculate and update total value
          const totalValue = calculateGFN(params);
          document.querySelector('.total-value').textContent = formatDollarValue(totalValue);
        }
        
        function setPreset(preset) {
          const presets = {
            openai: ${JSON.stringify(OPENAI_PARAMS)},
            anthropic: ${JSON.stringify(ANTHROPIC_PARAMS)},
            openagents_current: ${JSON.stringify(OPENAGENTS_PROJECTIONS.current)},
            openagents_6m: ${JSON.stringify(OPENAGENTS_PROJECTIONS.sixMonths)},
            openagents_1y: ${JSON.stringify(OPENAGENTS_PROJECTIONS.oneYear)},
            openagents_2y: ${JSON.stringify(OPENAGENTS_PROJECTIONS.twoYears)},
            openagents_5y: ${JSON.stringify(OPENAGENTS_PROJECTIONS.fiveYears)}
          };
          
          const params = presets[preset];
          if (!params) return;
          
          Object.entries(params).forEach(([key, value]) => {
            const input = document.getElementById(key);
            if (input) {
              input.value = value;
              // Update display value
              const display = document.getElementById(key + '_value');
              if (display) {
                display.textContent = formatValue(key, value);
              }
            }
          });
          
          updateGFN();
        }
        
        // Update display values on slider change
        document.addEventListener('DOMContentLoaded', () => {
          document.querySelectorAll('input[type="range"]').forEach(input => {
            input.addEventListener('input', (e) => {
              const key = e.target.id;
              const value = parseFloat(e.target.value);
              const display = document.getElementById(key + '_value');
              if (display) {
                display.textContent = formatValue(key, value);
              }
              updateGFN();
            });
          });
        });
      </script>
    `
  })
}
