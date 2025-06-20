import { html } from "@openagentsinc/psionic"

export interface SpawnAgentFormProps {
  onSpawn?: (name: string, initialCapital: number) => void
}

export function spawnAgentForm({ onSpawn: _onSpawn }: SpawnAgentFormProps = {}) {
  return html`
    <div class="spawn-form" box-="square">
      <h3>Spawn New Agent</h3>
      
      <form id="spawn-agent-form" onsubmit="return handleSpawnAgent(event)">
        <div class="form-group">
          <label for="agent-name">Agent Name</label>
          <input 
            is-="input" 
            type="text" 
            id="agent-name" 
            name="name"
            placeholder="My Agent" 
            box-="square"
            required
          >
        </div>
        
        <div class="form-group">
          <label for="initial-capital">Initial Capital (sats)</label>
          <input 
            is-="input" 
            type="number" 
            id="initial-capital" 
            name="capital"
            min="0" 
            value="10000" 
            placeholder="10000" 
            box-="square"
            required
          >
          <small>Agent needs capital to pay for metabolic costs</small>
        </div>
        
        <div class="form-group">
          <label for="metabolic-rate">Metabolic Rate (sats/hour)</label>
          <input 
            is-="input" 
            type="number" 
            id="metabolic-rate" 
            name="metabolicRate"
            min="1" 
            value="100" 
            placeholder="100" 
            box-="square"
            required
          >
          <small>Operational cost per hour</small>
        </div>
        
        <div class="form-actions">
          <button is-="button" type="submit" variant-="foreground1" box-="square">
            Spawn Agent
          </button>
          <button is-="button" type="button" variant-="background1" box-="square" onclick="resetSpawnForm()">
            Reset
          </button>
        </div>
      </form>
    </div>

    <script>
      function handleSpawnAgent(event) {
        event.preventDefault();
        const formData = new FormData(event.target);
        const name = formData.get('name');
        const capital = parseInt(formData.get('capital'));
        const metabolicRate = parseInt(formData.get('metabolicRate'));
        
        // Dispatch custom event that the homepage can listen to
        window.dispatchEvent(new CustomEvent('spawn-agent', {
          detail: { name, capital, metabolicRate }
        }));
        
        // Reset form
        event.target.reset();
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
