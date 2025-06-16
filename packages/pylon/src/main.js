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
const mnemonic = Agent.generateMnemonic();
console.log(`   🎯 Mnemonic: ${mnemonic}`);

(async () => {
  try {
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
  const statusDot = document.getElementById('ollama-status-dot');
  const statusText = document.getElementById('ollama-status-text');
  const modelInfo = document.getElementById('ollama-model-info');
  const modelListCard = document.getElementById('model-list-card');
  const modelList = document.getElementById('model-list');

  // Remove all status classes
  statusDot.classList.remove('checking', 'online', 'offline');

  if (status.online) {
    statusDot.classList.add('online');
    statusText.textContent = 'Online';

    // Show model count if available
    if (status.modelCount > 0) {
      // modelInfo.style.display = 'block';
      // modelInfo.querySelector('span').textContent = `${status.modelCount} model${status.modelCount !== 1 ? 's' : ''} available`;

      // Display model list
      modelListCard.style.display = 'block';
      modelList.innerHTML = '';

      status.models.forEach(model => {
        const modelItem = document.createElement('div');
        modelItem.className = 'model-item';

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

        modelItem.appendChild(modelName);
        modelItem.appendChild(modelDetails);
        modelList.appendChild(modelItem);
      });
    } else {
      modelInfo.style.display = 'none';
      modelListCard.style.display = 'none';
    }
  } else {
    statusDot.classList.add('offline');
    statusText.textContent = 'Offline';
    modelInfo.style.display = 'none';
    modelListCard.style.display = 'none';
  }
};

// Check Ollama status on load (legacy functionality)
const checkOllamaStatus = async () => {
  const statusDot = document.getElementById('ollama-status-dot');
  if (statusDot) {
    statusDot.classList.add('checking');

    try {
      const status = await checkOllama();
      updateOllamaStatus(status);
    } catch (error) {
      console.error('Error checking Ollama status:', error);
      updateOllamaStatus({ online: false });
    }
  }
};

// Initial check
checkOllamaStatus();

// Poll every 10 seconds
setInterval(checkOllamaStatus, 10000);

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
