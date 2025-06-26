/**
 * Project Coordination Test Page
 * Demonstrates multi-agent project task coordination and monitoring
 */

import { html } from "@openagentsinc/psionic"
// import type * as SDK from "@openagentsinc/sdk"
import { navigation } from "../../components/navigation.js"

export default () => {
  return html`
    <!doctype html>
    <html lang="en" variant-="catppuccin">
      <head>
        <meta charset="utf-8" />
        <title>Project Coordination Test - OpenAgents</title>
        <link rel="stylesheet" href="/css/main.css" />
        <link rel="stylesheet" href="/css/webtui.css" />
        <script src="/js/theme.js"></script>
      </head>
      <body>
        ${navigation({ current: "test" })}
        
        <main>
          <div class="container">
            <h1>Project Coordination Test</h1>
            <p>Monitor and coordinate multi-agent project execution</p>
            
            <section box-="double">
              <h2>Coalition Info</h2>
              <div id="coalition-info">
                <p>Loading coalition information...</p>
              </div>
            </section>
            
            <section box-="single">
              <h2>Project Coordination Process</h2>
              
              <div id="coordination-status">
                <p>Waiting for coalition information...</p>
              </div>
              
              <div class="button-group">
                <button is-="button" variant-="foreground1" box-="double" onclick="decomposeProject()" disabled>
                  1. Decompose Project
                </button>
                
                <button is-="button" variant-="foreground1" box-="double" onclick="assignTasks()" disabled>
                  2. Assign Tasks
                </button>
                
                <button is-="button" variant-="foreground1" box-="double" onclick="optimizeSchedule()" disabled>
                  3. Optimize Schedule
                </button>
                
                <button is-="button" variant-="foreground1" box-="double" onclick="startMonitoring()" disabled>
                  4. Start Progress Monitoring
                </button>
              </div>
            </section>
            
            <section box-="single" id="project-plan" style="display: none;">
              <h2>Project Plan</h2>
              <div id="project-plan-content"></div>
            </section>
            
            <section box-="single" id="task-assignments" style="display: none;">
              <h2>Task Assignments</h2>
              <div id="task-assignments-content"></div>
            </section>
            
            <section box-="single" id="project-schedule" style="display: none;">
              <h2>Project Schedule</h2>
              <div id="project-schedule-content"></div>
            </section>
            
            <section box-="single" id="progress-monitor" style="display: none;">
              <h2>Progress Monitor</h2>
              <div id="progress-content">
                <div class="progress-bar" box-="single">
                  <div class="progress-fill" style="width: 0%"></div>
                </div>
                <p>Progress: <span id="progress-percent">0%</span></p>
                <div id="progress-details"></div>
              </div>
            </section>
            
            <section box-="single" id="trust-network" style="display: none;">
              <h2>Trust & Reputation</h2>
              <div id="trust-content"></div>
            </section>
          </div>
        </main>
        
        <script>
          // Store state
          let coalition = null;
          let projectPlan = null;
          let taskAssignments = null;
          let projectSchedule = null;
          let monitoringInterval = null;
          
          // Load coalition info on page load
          async function loadCoalition() {
            const params = new URLSearchParams(window.location.search);
            const coalitionId = params.get('coalitionId');
            
            if (!coalitionId) {
              // Use mock coalition for demo
              coalition = {
                coalitionId: 'demo_coalition_001',
                contract: {
                  proposal: {
                    project: {
                      id: 'demo_project_001',
                      title: 'E-commerce Platform Development',
                      requirements: [
                        { skill: 'backend-development', priority: 'required', estimatedTokens: 50000, estimatedDurationHours: 40 },
                        { skill: 'frontend-development', priority: 'required', estimatedTokens: 40000, estimatedDurationHours: 35 },
                        { skill: 'ai-integration', priority: 'required', estimatedTokens: 30000, estimatedDurationHours: 20 }
                      ]
                    }
                  }
                },
                members: [
                  { agentId: 'backend_dev_001', capabilities: ['backend-development', 'testing'], averageRating: 4.8 },
                  { agentId: 'frontend_dev_001', capabilities: ['frontend-development', 'documentation'], averageRating: 4.6 },
                  { agentId: 'ai_specialist_001', capabilities: ['ai-integration', 'optimization'], averageRating: 4.9 }
                ],
                projectProgress: 0,
                activeTasks: [],
                completedTasks: []
              };
              
              displayCoalitionInfo();
              document.querySelector('button[onclick="decomposeProject()"]').disabled = false;
            } else {
              // TODO: Load real coalition from API
              updateStatus('Loading coalition ' + coalitionId + '...', 'loading');
            }
          }
          
          function displayCoalitionInfo() {
            const infoDiv = document.getElementById('coalition-info');
            infoDiv.innerHTML = \`
              <dl>
                <dt>Coalition ID:</dt>
                <dd>\${coalition.coalitionId}</dd>
                
                <dt>Project:</dt>
                <dd>\${coalition.contract.proposal.project.title}</dd>
                
                <dt>Members:</dt>
                <dd>\${coalition.members.length} agents</dd>
                
                <dt>Progress:</dt>
                <dd>\${Math.round(coalition.projectProgress * 100)}%</dd>
              </dl>
              
              <h3>Coalition Members</h3>
              <table>
                <thead>
                  <tr>
                    <th>Agent ID</th>
                    <th>Capabilities</th>
                    <th>Rating</th>
                  </tr>
                </thead>
                <tbody>
                  \${coalition.members.map(member => \`
                    <tr>
                      <td>\${member.agentId}</td>
                      <td>\${member.capabilities.join(', ')}</td>
                      <td>⭐ \${member.averageRating}</td>
                    </tr>
                  \`).join('')}
                </tbody>
              </table>
            \`;
          }
          
          async function updateStatus(message, type = 'info') {
            const statusDiv = document.getElementById('coordination-status');
            statusDiv.innerHTML = \`<p class="\${type}">\${message}</p>\`;
          }
          
          async function decomposeProject() {
            updateStatus('Decomposing project into tasks...', 'loading');
            
            try {
              const response = await fetch('/api/coalition/decompose-project', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ project: coalition.contract.proposal.project })
              });
              
              const result = await response.json();
              
              if (result.success) {
                projectPlan = result.plan;
                updateStatus('Project decomposed into ' + projectPlan.tasks.length + ' tasks', 'success');
                displayProjectPlan();
                document.querySelector('button[onclick="assignTasks()"]').disabled = false;
              } else {
                updateStatus('Failed to decompose project: ' + result.error, 'error');
              }
            } catch (error) {
              updateStatus('Error: ' + error.message, 'error');
            }
          }
          
          function displayProjectPlan() {
            const section = document.getElementById('project-plan');
            const content = document.getElementById('project-plan-content');
            section.style.display = 'block';
            
            content.innerHTML = \`
              <dl>
                <dt>Total Tasks:</dt>
                <dd>\${projectPlan.tasks.length}</dd>
                
                <dt>Estimated Duration:</dt>
                <dd>\${projectPlan.estimatedDuration} hours</dd>
                
                <dt>Parallelization Factor:</dt>
                <dd>\${projectPlan.parallelizationFactor}x</dd>
              </dl>
              
              <h3>Tasks</h3>
              <table>
                <thead>
                  <tr>
                    <th>Task</th>
                    <th>Required Skills</th>
                    <th>Est. Hours</th>
                    <th>Dependencies</th>
                  </tr>
                </thead>
                <tbody>
                  \${projectPlan.tasks.map(task => \`
                    <tr>
                      <td>\${task.title}</td>
                      <td>\${task.requiredSkills.join(', ')}</td>
                      <td>\${task.estimatedHours}h</td>
                      <td>\${task.dependencies.length > 0 ? task.dependencies.length + ' deps' : 'None'}</td>
                    </tr>
                  \`).join('')}
                </tbody>
              </table>
              
              <h3>Milestones</h3>
              <ul>
                \${projectPlan.milestones.map(milestone => \`
                  <li>\${milestone.name} - \${new Date(milestone.targetDate).toLocaleDateString()}</li>
                \`).join('')}
              </ul>
            \`;
          }
          
          async function assignTasks() {
            updateStatus('Assigning tasks to coalition members...', 'loading');
            
            try {
              const response = await fetch('/api/coalition/assign-tasks', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ plan: projectPlan, coalition: coalition })
              });
              
              const result = await response.json();
              
              if (result.success) {
                taskAssignments = result.assignments;
                updateStatus('Tasks assigned successfully', 'success');
                displayTaskAssignments();
                document.querySelector('button[onclick="optimizeSchedule()"]').disabled = false;
              } else {
                updateStatus('Failed to assign tasks: ' + result.error, 'error');
              }
            } catch (error) {
              updateStatus('Error: ' + error.message, 'error');
            }
          }
          
          function displayTaskAssignments() {
            const section = document.getElementById('task-assignments');
            const content = document.getElementById('task-assignments-content');
            section.style.display = 'block';
            
            // Group assignments by agent
            const agentTasks = {};
            Object.entries(taskAssignments.assignments).forEach(([taskId, agentId]) => {
              if (!agentTasks[agentId]) agentTasks[agentId] = [];
              const task = projectPlan.tasks.find(t => t.taskId === taskId);
              if (task) agentTasks[agentId].push(task);
            });
            
            content.innerHTML = \`
              <h3>Workload Distribution</h3>
              <table>
                <thead>
                  <tr>
                    <th>Agent</th>
                    <th>Assigned Tasks</th>
                    <th>Total Hours</th>
                  </tr>
                </thead>
                <tbody>
                  \${Object.entries(taskAssignments.workload).map(([agentId, hours]) => \`
                    <tr>
                      <td>\${agentId}</td>
                      <td>\${agentTasks[agentId]?.length || 0} tasks</td>
                      <td>\${hours}h</td>
                    </tr>
                  \`).join('')}
                </tbody>
              </table>
              
              \${taskAssignments.conflicts.length > 0 ? \`
                <div class="warning">
                  <h4>Conflicts</h4>
                  <p>\${taskAssignments.conflicts.length} tasks could not be assigned</p>
                </div>
              \` : ''}
              
              \${taskAssignments.recommendations.length > 0 ? \`
                <div class="info">
                  <h4>Recommendations</h4>
                  <ul>
                    \${taskAssignments.recommendations.map(rec => \`<li>\${rec}</li>\`).join('')}
                  </ul>
                </div>
              \` : ''}
            \`;
          }
          
          async function optimizeSchedule() {
            updateStatus('Optimizing project schedule...', 'loading');
            
            try {
              const response = await fetch('/api/coalition/optimize-schedule', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ assignments: taskAssignments })
              });
              
              const result = await response.json();
              
              if (result.success) {
                projectSchedule = result.schedule;
                updateStatus('Schedule optimized', 'success');
                displayProjectSchedule();
                document.querySelector('button[onclick="startMonitoring()"]').disabled = false;
              } else {
                updateStatus('Failed to optimize schedule: ' + result.error, 'error');
              }
            } catch (error) {
              updateStatus('Error: ' + error.message, 'error');
            }
          }
          
          function displayProjectSchedule() {
            const section = document.getElementById('project-schedule');
            const content = document.getElementById('project-schedule-content');
            section.style.display = 'block';
            
            const duration = projectSchedule.endTime - projectSchedule.startTime;
            const days = Math.ceil(duration / (24 * 60 * 60 * 1000));
            
            content.innerHTML = \`
              <dl>
                <dt>Start Date:</dt>
                <dd>\${new Date(projectSchedule.startTime).toLocaleDateString()}</dd>
                
                <dt>End Date:</dt>
                <dd>\${new Date(projectSchedule.endTime).toLocaleDateString()}</dd>
                
                <dt>Duration:</dt>
                <dd>\${days} days</dd>
                
                <dt>Buffer Time:</dt>
                <dd>\${Math.round(projectSchedule.bufferTime / (60 * 60 * 1000))} hours</dd>
                
                <dt>Risk Score:</dt>
                <dd>\${Math.round(projectSchedule.riskScore * 100)}%</dd>
              </dl>
              
              <h3>Gantt Chart</h3>
              <div class="gantt-chart" box-="single">
                \${visualizeSchedule()}
              </div>
            \`;
          }
          
          function visualizeSchedule() {
            // Simple ASCII gantt chart
            const startTime = projectSchedule.startTime;
            const totalDuration = projectSchedule.endTime - startTime;
            const chartWidth = 50;
            
            return projectSchedule.taskTimeline.map(item => {
              const task = projectPlan.tasks.find(t => t.taskId === item.taskId);
              const relativeStart = (item.startTime - startTime) / totalDuration;
              const relativeDuration = (item.endTime - item.startTime) / totalDuration;
              
              const startPos = Math.floor(relativeStart * chartWidth);
              const barLength = Math.max(1, Math.floor(relativeDuration * chartWidth));
              
              const bar = ' '.repeat(startPos) + '█'.repeat(barLength);
              
              return \`<div><code>\${item.agentId.padEnd(20)} |\${bar}</code></div>\`;
            }).join('');
          }
          
          async function startMonitoring() {
            updateStatus('Starting progress monitoring...', 'loading');
            document.getElementById('progress-monitor').style.display = 'block';
            
            // Simulate progress updates
            let progress = 0;
            monitoringInterval = setInterval(async () => {
              progress += Math.random() * 10;
              if (progress > 100) progress = 100;
              
              updateProgress(progress);
              
              if (progress >= 100) {
                clearInterval(monitoringInterval);
                updateStatus('Project completed!', 'success');
                await showTrustNetwork();
              }
            }, 2000);
            
            updateStatus('Monitoring active - simulating progress...', 'info');
          }
          
          function updateProgress(percent) {
            document.querySelector('.progress-fill').style.width = percent + '%';
            document.getElementById('progress-percent').textContent = Math.round(percent) + '%';
            
            const detailsDiv = document.getElementById('progress-details');
            const completed = Math.floor((projectPlan.tasks.length * percent) / 100);
            
            detailsDiv.innerHTML = \`
              <dl>
                <dt>Tasks Completed:</dt>
                <dd>\${completed} / \${projectPlan.tasks.length}</dd>
                
                <dt>Active Agents:</dt>
                <dd>\${coalition.members.length}</dd>
                
                <dt>Estimated Completion:</dt>
                <dd>\${percent < 100 ? 'In progress...' : 'Completed'}</dd>
              </dl>
            \`;
          }
          
          async function showTrustNetwork() {
            document.getElementById('trust-network').style.display = 'block';
            
            try {
              const response = await fetch('/api/coalition/trust-analysis', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ coalition: coalition })
              });
              
              const result = await response.json();
              
              if (result.success) {
                displayTrustNetwork(result.analysis);
              }
            } catch (error) {
              console.error('Failed to get trust analysis:', error);
            }
          }
          
          function displayTrustNetwork(analysis) {
            const content = document.getElementById('trust-content');
            
            content.innerHTML = \`
              <h3>Coalition Trust Analysis</h3>
              <dl>
                <dt>Coalition Success Prediction:</dt>
                <dd>\${Math.round(analysis.successPrediction * 100)}%</dd>
                
                <dt>Average Trust Level:</dt>
                <dd>\${Math.round(analysis.avgTrust * 100)}%</dd>
              </dl>
              
              <h3>Agent Reputations</h3>
              <table>
                <thead>
                  <tr>
                    <th>Agent</th>
                    <th>Quality</th>
                    <th>Timeliness</th>
                    <th>Collaboration</th>
                    <th>Overall</th>
                  </tr>
                </thead>
                <tbody>
                  \${analysis.agentReputations.map(rep => \`
                    <tr>
                      <td>\${rep.agentId}</td>
                      <td>⭐ \${rep.quality.toFixed(1)}</td>
                      <td>⭐ \${rep.timeliness.toFixed(1)}</td>
                      <td>⭐ \${rep.collaboration.toFixed(1)}</td>
                      <td>⭐ \${rep.overall.toFixed(1)}</td>
                    </tr>
                  \`).join('')}
                </tbody>
              </table>
            \`;
          }
          
          // Load coalition on page load
          loadCoalition();
        </script>
      </body>
    </html>
  `
}
