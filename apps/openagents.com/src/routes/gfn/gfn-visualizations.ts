/**
 * GFN Visualization Components
 * Simple ASCII-based charts for network effect visualization
 */

import { GFNResults, GFNParameters, formatValue } from './gfn-calculator.js';

/**
 * Create a simple bar chart showing network effect breakdown
 */
export function createNetworkEffectChart(results: GFNResults): string {
  const maxWidth = 40;
  const { percentages } = results;
  
  const sarnoffBar = '█'.repeat(Math.round(percentages.sarnoff * maxWidth / 100));
  const metcalfeBar = '█'.repeat(Math.round(percentages.metcalfe * maxWidth / 100));
  const reedBar = '█'.repeat(Math.round(percentages.reed * maxWidth / 100));
  
  return `
<div class="network-effect-chart" style="font-family: monospace; line-height: 1.5;">
  <h3 style="margin-bottom: 1rem;">Network Effect Breakdown</h3>
  <div style="margin-bottom: 0.5rem;">
    <div style="display: flex; align-items: center;">
      <span style="width: 120px;">Sarnoff:</span>
      <span style="color: var(--foreground2);">${sarnoffBar}</span>
      <span style="margin-left: 0.5rem;">${percentages.sarnoff.toFixed(1)}%</span>
    </div>
  </div>
  <div style="margin-bottom: 0.5rem;">
    <div style="display: flex; align-items: center;">
      <span style="width: 120px;">Metcalfe:</span>
      <span style="color: var(--foreground1);">${metcalfeBar}</span>
      <span style="margin-left: 0.5rem;">${percentages.metcalfe.toFixed(1)}%</span>
    </div>
  </div>
  <div style="margin-bottom: 0.5rem;">
    <div style="display: flex; align-items: center;">
      <span style="width: 120px;">Reed:</span>
      <span style="color: var(--foreground0);">${reedBar}</span>
      <span style="margin-left: 0.5rem;">${percentages.reed.toFixed(1)}%</span>
    </div>
  </div>
</div>
  `;
}

/**
 * Create value breakdown display
 */
export function createValueBreakdown(results: GFNResults, params: GFNParameters): string {
  const multiplierEffect = results.totalValue / results.baseValue;
  
  return `
<div class="value-breakdown" style="font-family: monospace;">
  <h3 style="margin-bottom: 1rem;">Value Components</h3>
  <table style="width: 100%; border-collapse: collapse;">
    <tr>
      <td style="padding: 0.25rem 0;">Sarnoff (Linear):</td>
      <td style="text-align: right; color: var(--foreground2);">${formatValue(results.sarnoffValue)}</td>
    </tr>
    <tr>
      <td style="padding: 0.25rem 0;">Metcalfe (Quadratic):</td>
      <td style="text-align: right; color: var(--foreground1);">${formatValue(results.metcalfeValue)}</td>
    </tr>
    <tr>
      <td style="padding: 0.25rem 0;">Reed (Exponential):</td>
      <td style="text-align: right; color: var(--foreground0);">${formatValue(results.reedValue)}</td>
    </tr>
    <tr style="border-top: 1px solid var(--foreground2);">
      <td style="padding: 0.5rem 0 0.25rem 0;">Base Value:</td>
      <td style="text-align: right; padding-top: 0.5rem;">${formatValue(results.baseValue)}</td>
    </tr>
    <tr>
      <td style="padding: 0.25rem 0;">Multipliers (Q×M×(1+D)):</td>
      <td style="text-align: right;">${multiplierEffect.toFixed(2)}x</td>
    </tr>
    <tr style="border-top: 2px solid var(--foreground0);">
      <td style="padding: 0.5rem 0; font-weight: bold;">Total Value:</td>
      <td style="text-align: right; padding-top: 0.5rem; font-weight: bold; color: var(--foreground0);">${formatValue(results.totalValue)}</td>
    </tr>
  </table>
</div>
  `;
}

/**
 * Create a simple line chart for projections
 */
export function createProjectionChart(
  projections: { label: string; n: number; value: number }[]
): string {
  if (projections.length === 0) return '';
  
  const maxValue = Math.max(...projections.map(p => p.value));
  const chartHeight = 10;
  const chartWidth = 50;
  
  // Create ASCII chart
  const chart: string[][] = Array(chartHeight).fill(null).map(() => 
    Array(chartWidth).fill(' ')
  );
  
  // Add axes
  for (let i = 0; i < chartHeight; i++) {
    chart[i][0] = '│';
  }
  for (let i = 0; i < chartWidth; i++) {
    chart[chartHeight - 1][i] = '─';
  }
  chart[chartHeight - 1][0] = '└';
  
  // Plot points
  projections.forEach((point, index) => {
    const x = Math.floor((index / (projections.length - 1)) * (chartWidth - 2)) + 1;
    const y = chartHeight - 2 - Math.floor((point.value / maxValue) * (chartHeight - 2));
    if (y >= 0 && y < chartHeight && x >= 0 && x < chartWidth) {
      chart[y][x] = '●';
      
      // Connect points with lines (simple approximation)
      if (index > 0) {
        const prevPoint = projections[index - 1];
        const prevX = Math.floor(((index - 1) / (projections.length - 1)) * (chartWidth - 2)) + 1;
        const prevY = chartHeight - 2 - Math.floor((prevPoint.value / maxValue) * (chartHeight - 2));
        
        // Draw simple line
        const steps = Math.abs(x - prevX);
        for (let step = 1; step < steps; step++) {
          const interpX = prevX + step;
          const interpY = Math.round(prevY + (y - prevY) * (step / steps));
          if (interpY >= 0 && interpY < chartHeight && interpX >= 0 && interpX < chartWidth) {
            chart[interpY][interpX] = '·';
          }
        }
      }
    }
  });
  
  // Convert to string
  const chartStr = chart.map(row => row.join('')).join('\n');
  
  // Create labels
  const labels = projections.map(p => p.label).join('    ');
  
  return `
<div class="projection-chart" style="font-family: monospace;">
  <h3 style="margin-bottom: 1rem;">Growth Projections</h3>
  <pre style="line-height: 1;">${chartStr}</pre>
  <div style="margin-top: 0.5rem; font-size: 0.8em;">
    ${labels}
  </div>
  <div style="margin-top: 1rem;">
    ${projections.map(p => `
      <div style="margin-bottom: 0.25rem;">
        <strong>${p.label}:</strong> ${formatValue(p.value)} (${p.n.toLocaleString()} agents)
      </div>
    `).join('')}
  </div>
</div>
  `;
}

/**
 * Create dominant effect indicator
 */
export function createDominantEffectIndicator(results: GFNResults, n: number): string {
  const indicators = {
    sarnoff: {
      symbol: '📡',
      name: "Sarnoff's Law",
      description: 'Broadcast value dominates',
      range: 'n < 100'
    },
    metcalfe: {
      symbol: '🔗',
      name: "Metcalfe's Law", 
      description: 'P2P connections dominate',
      range: '100 < n < 10,000'
    },
    reed: {
      symbol: '🚀',
      name: "Reed's Law",
      description: 'Group formation dominates',
      range: 'n > 10,000 with high clustering'
    }
  };
  
  const current = indicators[results.dominantEffect];
  
  return `
<div class="dominant-effect" style="text-align: center; padding: 1rem; border: 1px solid var(--foreground1); border-radius: 0.5rem;">
  <div style="font-size: 3rem; margin-bottom: 0.5rem;">${current.symbol}</div>
  <h3 style="margin-bottom: 0.5rem;">${current.name} Dominates</h3>
  <p style="margin-bottom: 0.5rem;">${current.description}</p>
  <p style="font-size: 0.9em; color: var(--foreground2);">Typical range: ${current.range}</p>
  <p style="font-size: 0.9em; color: var(--foreground1);">Current n: ${n.toLocaleString()}</p>
</div>
  `;
}