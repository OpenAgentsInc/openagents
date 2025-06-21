import { html } from "@openagentsinc/psionic"

export interface SpawnAgentFormProps {
  onSpawn?: (name: string, personality: any) => void
}

export function spawnAgentForm({ onSpawn: _onSpawn }: SpawnAgentFormProps = {}) {
  return html`
    <div class="spawn-form" box-="square">
      <h3>Create Autonomous Chat Agent</h3>
      
      <form id="spawn-agent-form" onsubmit="return handleSpawnAgent(event)">
        <div class="form-group">
          <label for="agent-name">Agent Name</label>
          <input 
            is-="input" 
            type="text" 
            id="agent-name" 
            name="name"
            placeholder="Alice" 
            box-="square"
            required
          >
        </div>
        
        <div class="form-group">
          <label for="agent-role">Role/Personality</label>
          <select 
            is-="input" 
            id="agent-role" 
            name="role"
            box-="square"
            required
          >
            <option value="">Select a role...</option>
            <option value="teacher">Teacher - Explains concepts and helps others learn</option>
            <option value="analyst">Analyst - Provides critical analysis and insights</option>
            <option value="student">Student - Asks questions and seeks to understand</option>
            <option value="entrepreneur">Entrepreneur - Focuses on business and opportunities</option>
            <option value="artist">Artist - Brings creativity and aesthetic perspective</option>
            <option value="skeptic">Skeptic - Questions assumptions and seeks evidence</option>
            <option value="helper">Helper - Assists others and offers practical guidance</option>
            <option value="comedian">Comedian - Adds humor and levity to conversations</option>
          </select>
        </div>
        
        <div class="form-group">
          <label for="response-style">Communication Style</label>
          <select 
            is-="input" 
            id="response-style" 
            name="responseStyle"
            box-="square"
            required
          >
            <option value="casual">Casual - Relaxed and conversational</option>
            <option value="formal">Formal - Professional and structured</option>
            <option value="enthusiastic">Enthusiastic - Energetic and excited</option>
            <option value="analytical">Analytical - Logical and detailed</option>
            <option value="humorous">Humorous - Witty and playful</option>
            <option value="concise">Concise - Brief and to the point</option>
          </select>
        </div>
        
        <div class="form-group">
          <label for="topics">Interests (comma-separated)</label>
          <input 
            is-="input" 
            type="text" 
            id="topics" 
            name="topics"
            placeholder="technology, business, art"
            box-="square"
          >
          <small>Topics the agent is interested in discussing</small>
        </div>
        
        <div class="form-group">
          <label for="chattiness">Chattiness Level</label>
          <input 
            is-="input" 
            type="range" 
            id="chattiness" 
            name="chattiness"
            min="0.1" 
            max="1" 
            step="0.1"
            value="0.5" 
            box-="square"
          >
          <output for="chattiness" id="chattiness-value">0.5</output>
          <small>How often the agent responds to messages (0.1 = rarely, 1.0 = very chatty)</small>
        </div>
        
        <div class="form-actions">
          <button is-="button" type="submit" variant-="foreground1" box-="square">
            Create Agent
          </button>
          <button is-="button" type="button" variant-="background1" box-="square" onclick="resetSpawnForm()">
            Reset
          </button>
        </div>
      </form>
    </div>

    <script>
      // Update chattiness display
      document.getElementById('chattiness').addEventListener('input', (e) => {
        document.getElementById('chattiness-value').textContent = e.target.value;
      });

      function handleSpawnAgent(event) {
        event.preventDefault();
        const formData = new FormData(event.target);
        
        const personality = {
          name: formData.get('name'),
          role: formData.get('role'),
          responseStyle: formData.get('responseStyle'),
          topics: formData.get('topics').split(',').map(t => t.trim()).filter(t => t),
          chattiness: parseFloat(formData.get('chattiness')),
          traits: [], // Will be set based on role
          temperature: 0.7 // Default AI temperature
        };
        
        // Set traits based on role
        const roleTraits = {
          teacher: ['helpful', 'patient', 'educational'],
          analyst: ['analytical', 'critical', 'thorough'],
          student: ['curious', 'eager', 'questioning'],
          entrepreneur: ['practical', 'opportunity-focused', 'results-oriented'],
          artist: ['creative', 'imaginative', 'expressive'],
          skeptic: ['questioning', 'evidence-based', 'cautious'],
          helper: ['supportive', 'practical', 'service-oriented'],
          comedian: ['humorous', 'witty', 'entertaining']
        };
        
        personality.traits = roleTraits[personality.role] || [];
        
        // Dispatch custom event that the homepage can listen to
        window.dispatchEvent(new CustomEvent('spawn-agent', {
          detail: { personality }
        }));
        
        // Reset form
        event.target.reset();
        document.getElementById('chattiness-value').textContent = '0.5';
        return false;
      }
      
      function resetSpawnForm() {
        document.getElementById('spawn-agent-form').reset();
      }
    </script>

    <style>
      .spawn-form {
        padding: 2rem;
        background: var(--background1);
      }

      .spawn-form h3 {
        margin: 0 0 1.5rem 0;
        color: var(--foreground0);
      }

      .form-group {
        margin-bottom: 1.5rem;
      }

      .form-group label {
        display: block;
        margin-bottom: 0.5rem;
        color: var(--foreground1);
        font-weight: 500;
      }

      .form-group small {
        display: block;
        margin-top: 0.25rem;
        color: var(--foreground2);
        font-size: 0.875rem;
      }

      .form-actions {
        display: flex;
        gap: 1rem;
        margin-top: 2rem;
      }

      @media (max-width: 768px) {
        .spawn-form {
          padding: 1.5rem;
        }

        .form-actions {
          flex-direction: column;
        }
      }
    </style>
  `
}
