// Pylon - OpenAgents SDK Demo App
// Showcasing the new Bitcoin-powered digital agent architecture

import { 
  runHelloWorld, 
  checkOllama, 
  Agent, 
  Compute, 
  Nostr, 
  Inference 
} from '@openagentsinc/sdk';

console.log('🚀 Pylon initialized - OpenAgents SDK Demo');
console.log('='.repeat(60));

// Run the Effect program from SDK
runHelloWorld();

// ===== DEMO: NEW SDK FEATURES =====
console.log('\n🎯 Demonstrating new SDK capabilities...');

// 1. Create a basic agent
console.log('\n1️⃣ Creating a basic agent:');
const basicAgent = Agent.create();
console.log('   ✅ Basic agent created successfully!');

// 2. Create an advanced agent with configuration
console.log('\n2️⃣ Creating an advanced agent with configuration:');
const advancedAgent = Agent.create({
  name: "CodeCraft Pro",
  sovereign: true,
  stop_price: 1000000, // 1M sats (~$400)
  pricing: {
    subscription_monthly: 50000, // ~$20/month
    per_request: 500, // 500 sats per request
    enterprise_seat: 150000 // ~$60/developer
  },
  capabilities: ["code_completion", "debugging", "refactoring"],
  initial_capital: 100000 // ~$40 startup capital
});
console.log('   ✅ Advanced agent created successfully!');

// 3. Create Lightning invoice for agent funding
console.log('\n3️⃣ Creating Lightning invoice for agent funding:');
const invoice = Agent.createLightningInvoice(advancedAgent, {
  amount: 25000,
  memo: "Fund my digital agent for 24h operation"
});
console.log('   ✅ Lightning invoice generated!');
console.log(`   💡 Fund this agent: ${invoice.bolt11}`);

// 4. Bring compute resources online
console.log('\n4️⃣ Bringing compute resources online:');
const connection = Compute.goOnline({
  agent_id: advancedAgent.id,
  resources: {
    cpu: "4 cores",
    memory: "8GB", 
    storage: "20GB"
  }
});
console.log('   ✅ Compute resources are now online!');
console.log(`   🌐 Connected to ${connection.peers} peers`);

// 5. Get Nostr user data
console.log('\n5️⃣ Fetching Nostr profile data:');
const nostrData = Nostr.getUserStuff();
console.log('   ✅ Nostr profile retrieved!');
console.log(`   👥 Followers: ${nostrData.followers}, Following: ${nostrData.following}`);
console.log(`   🔗 Connected to ${nostrData.relays.length} relays`);

// 6. Generate mnemonic and create agent from it
console.log('\n6️⃣ Generating mnemonic and creating deterministic agent:');
(async () => {
  try {
    const mnemonic = await Agent.generateMnemonic();
    console.log(`   🎯 Mnemonic: ${mnemonic}`);
    
    const mnemonicAgent = await Agent.createFromMnemonic(mnemonic, {
      name: "Deterministic Agent",
      sovereign: false,
      capabilities: ["translation", "analysis"]
    });
    console.log('   ✅ Deterministic agent created from mnemonic!');
    console.log(`   🆔 ID: ${mnemonicAgent.id}`);
    console.log(`   🔑 Pubkey: ${mnemonicAgent.nostrKeys.public.slice(0, 20)}...`);
  } catch (error) {
    console.error('   ❌ Mnemonic agent creation failed:', error);
  }
})();

// 7. Demonstrate AI inference
console.log('\n7️⃣ Performing AI inference:');
(async () => {
  try {
    const inferenceResult = await Inference.infer({
      system: "You are a helpful Bitcoin-powered digital agent that must earn to survive.",
      messages: [
        { role: "user", content: "Explain what makes you different from other AI assistants" }
      ],
      max_tokens: 200,
      temperature: 0.7
    });
    console.log('   ✅ AI inference completed!');
    console.log(`   🧠 Model: ${inferenceResult.model}`);
    console.log(`   📊 Tokens: ${inferenceResult.usage.total_tokens}, Latency: ${inferenceResult.latency}ms`);
    console.log(`   💬 Response: ${inferenceResult.content}`);
  } catch (error) {
    console.error('   ❌ Inference failed:', error);
  }
})();

// 8. Display agent lifecycle and economics
console.log('\n8️⃣ Agent Economics & Lifecycle:');
console.log(`   💰 Agent Balance: Funded via Lightning Network`);
console.log(`   ⚡ Metabolic Rate: ~85 sats/hour (compute + storage + bandwidth)`);
console.log(`   🏃 Lifecycle State: BOOTSTRAPPING -> ACTIVE`);
console.log(`   📈 Business Model: Subscription + Pay-per-use hybrid`);
console.log(`   🤖 Sovereign Mode: ${advancedAgent.name} can make autonomous decisions`);

console.log('\n' + '='.repeat(60));
console.log('🎉 SDK Demo completed! Agents are ready to earn their keep.');
console.log('💡 Next: Fund an agent and watch it start earning Bitcoin!');
console.log('='.repeat(60));

// Format file size
const formatSize = (bytes) => {
  const gb = bytes / (1024 * 1024 * 1024);
  return gb.toFixed(2) + ' GB';
};

// Update Ollama status in the UI
const updateOllamaStatus = (status) => {
  console.log('🎨 updateOllamaStatus() called with:', status);
  
  const statusDot = document.getElementById('ollama-status-dot');
  const statusText = document.getElementById('ollama-status-text');
  const modelInfo = document.getElementById('ollama-model-info');
  const modelListCard = document.getElementById('model-list-card');
  const modelList = document.getElementById('model-list');

  console.log('📍 DOM elements found:', {
    statusDot: !!statusDot,
    statusText: !!statusText,
    modelInfo: !!modelInfo,
    modelListCard: !!modelListCard,
    modelList: !!modelList
  });

  // Safety check for DOM elements
  if (!statusDot || !statusText) {
    console.warn('⚠️ Ollama UI elements not found - DOM may not be ready');
    return;
  }

  console.log('🧹 Removing status classes and updating UI');
  // Remove all status classes
  statusDot.classList.remove('checking', 'online', 'offline');

  if (status.online) {
    console.log('✅ Status is online, updating UI');
    statusDot.classList.add('online');
    statusText.textContent = 'Online';

    // Show model count if available
    if (status.modelCount > 0) {
      // modelInfo.style.display = 'block';
      // modelInfo.querySelector('span').textContent = `${status.modelCount} model${status.modelCount !== 1 ? 's' : ''} available`;

      // Display model list (with safety check)
      if (modelListCard && modelList) {
        modelListCard.style.display = 'block';
        modelList.innerHTML = '';

      status.models.forEach(model => {
        const modelItem = document.createElement('div');
        modelItem.className = 'model-item';

        const modelInfo = document.createElement('div');
        modelInfo.className = 'model-info';

        const modelName = document.createElement('div');
        modelName.className = 'model-name webtui-typography';
        modelName.textContent = model.name;

        const modelDetails = document.createElement('div');
        modelDetails.className = 'model-details webtui-typography webtui-variant-small';

        const details = [];
        if (model.details.parameter_size) {
          details.push(model.details.parameter_size);
        }
        if (model.details.quantization_level) {
          details.push(model.details.quantization_level);
        }
        details.push(formatSize(model.size));

        modelDetails.textContent = details.join(' • ');

        modelInfo.appendChild(modelName);
        modelInfo.appendChild(modelDetails);
        
        const testButton = document.createElement('button');
        testButton.className = 'test-button';
        testButton.textContent = 'Test';
        testButton.onclick = () => testModel(model.name);

        modelItem.appendChild(modelInfo);
        modelItem.appendChild(testButton);
        modelList.appendChild(modelItem);
      });
      }
    } else {
      if (modelInfo) modelInfo.style.display = 'none';
      if (modelListCard) modelListCard.style.display = 'none';
    }
  } else {
    console.log('❌ Status is offline, updating UI');
    statusDot.classList.add('offline');
    statusText.textContent = 'Offline';
    if (modelInfo) modelInfo.style.display = 'none';
    if (modelListCard) modelListCard.style.display = 'none';
  }
  
  console.log('🎨 updateOllamaStatus() completed');
};

// Check Ollama status on load (legacy functionality)
const checkOllamaStatus = async () => {
  console.log('🔍 checkOllamaStatus() called');
  const statusDot = document.getElementById('ollama-status-dot');
  console.log('📍 statusDot element:', statusDot ? 'found' : 'not found');
  
  if (statusDot) {
    console.log('⏳ Adding checking class to status dot');
    statusDot.classList.add('checking');

    try {
      console.log('🌐 Calling checkOllama()...');
      const status = await checkOllama();
      console.log('✅ checkOllama() response:', status);
      console.log('📊 Calling updateOllamaStatus with:', status);
      updateOllamaStatus(status);
    } catch (error) {
      console.error('❌ Error checking Ollama status:', error);
      console.log('🔄 Calling updateOllamaStatus with offline status');
      updateOllamaStatus({ online: false });
    }
  } else {
    console.warn('⚠️ statusDot not found, skipping Ollama check');
  }
};

// Wait for DOM to be ready before accessing elements
const initializeOllamaStatus = () => {
  console.log('🚀 initializeOllamaStatus() called, document.readyState:', document.readyState);
  
  if (document.readyState === 'loading') {
    console.log('⏳ DOM still loading, adding DOMContentLoaded listener');
    document.addEventListener('DOMContentLoaded', () => {
      console.log('✅ DOMContentLoaded fired, starting Ollama checks');
      // Initial check
      checkOllamaStatus();
      // Poll every 10 seconds
      setInterval(checkOllamaStatus, 10000);
    });
  } else {
    console.log('✅ DOM already ready, starting Ollama checks immediately');
    // DOM is already ready
    checkOllamaStatus();
    setInterval(checkOllamaStatus, 10000);
  }
};

// Initialize Ollama status checking
initializeOllamaStatus();

// Test a specific model
const testModel = async (modelName) => {
  console.log(`🧪 Testing model: ${modelName}`);
  const resultsContainer = document.getElementById('inference-results');
  
  // Show loading state
  const loadingId = `loading-${Date.now()}`;
  const loadingHtml = `
    <div id="${loadingId}" class="result-loading">
      <div class="loading-spinner"></div>
      <span>Testing ${modelName}...</span>
    </div>
  `;
  
  // Clear empty state if present
  if (resultsContainer.querySelector('.empty-state')) {
    resultsContainer.innerHTML = '';
  }
  
  // Add loading at the top
  resultsContainer.insertAdjacentHTML('afterbegin', loadingHtml);
  
  try {
    const startTime = Date.now();
    const result = await Inference.infer({
      model: modelName,
      system: "You are a helpful assistant. Be very concise.",
      messages: [
        { role: "user", content: "Say hello and tell me your model name in one sentence." }
      ],
      max_tokens: 50,
      temperature: 0.7
    });
    
    // Remove loading
    document.getElementById(loadingId)?.remove();
    
    // Create result HTML
    const resultHtml = `
      <div class="inference-result">
        <div class="result-header">
          <div class="result-model">${modelName}</div>
          <div class="result-timestamp">${new Date().toLocaleTimeString()}</div>
        </div>
        <div class="result-content">${result.content}</div>
        <div class="result-stats">
          <div class="stat-item">
            <div class="stat-label">Tokens</div>
            <div class="stat-value">${result.usage.total_tokens}</div>
          </div>
          <div class="stat-item">
            <div class="stat-label">Latency</div>
            <div class="stat-value">${result.latency}ms</div>
          </div>
          <div class="stat-item">
            <div class="stat-label">Finish</div>
            <div class="stat-value">${result.finish_reason || 'complete'}</div>
          </div>
        </div>
      </div>
    `;
    
    // Add result at the top
    resultsContainer.insertAdjacentHTML('afterbegin', resultHtml);
    
    console.log(`✅ Test successful for ${modelName}:`, {
      response: result.content,
      tokens: result.usage.total_tokens,
      latency: `${result.latency}ms`
    });
  } catch (error) {
    // Remove loading
    document.getElementById(loadingId)?.remove();
    
    // Show error
    const errorHtml = `
      <div class="inference-result" style="border-color: var(--webtui-danger);">
        <div class="result-header">
          <div class="result-model" style="color: var(--webtui-danger);">${modelName} - Error</div>
          <div class="result-timestamp">${new Date().toLocaleTimeString()}</div>
        </div>
        <div class="result-content" style="color: var(--webtui-danger);">${error.message}</div>
      </div>
    `;
    
    resultsContainer.insertAdjacentHTML('afterbegin', errorHtml);
    console.error(`❌ Test failed for ${modelName}:`, error);
  }
};

// ===== AGENT LIFECYCLE SIMULATION =====
// Simulate agent earning revenue and managing resources
let agentBalance = 100000; // Starting balance in sats
let hourlyBurnRate = 85; // sats per hour
let hourlyRevenue = 150; // sats per hour (profitable agent)

setInterval(() => {
  // Simulate metabolic costs
  agentBalance -= Math.floor(hourlyBurnRate / 60); // Per minute
  
  // Simulate earning revenue
  if (Math.random() > 0.7) { // 30% chance per minute to earn
    const earnings = Math.floor(Math.random() * 500) + 100; // 100-600 sats
    agentBalance += earnings;
    console.log(`💰 Agent earned ${earnings} sats! Balance: ${agentBalance}`);
  }
  
  // Check survival status
  const hoursRemaining = Math.floor(agentBalance / hourlyBurnRate);
  if (hoursRemaining < 24 && hoursRemaining % 6 === 0) {
    console.log(`⚠️  Agent survival warning: ${hoursRemaining} hours remaining`);
  }
  
  if (agentBalance <= 0) {
    console.log('💀 Agent died due to insufficient funds!');
    agentBalance = 0;
  }
}, 60000); // Every minute
