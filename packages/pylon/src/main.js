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

// 7. Demonstrate AI inference (commented out to prevent automatic inference)
console.log('\n7️⃣ AI inference capability available - use the chat interface to test');
// Removed automatic inference demo

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

// Chat state management
let chatMessages = [];
let currentModel = '';
let isStreaming = false;

// Update Ollama status in the UI
const updateOllamaStatus = (status) => {
  console.log('🎨 updateOllamaStatus() called with:', status);
  
  const statusDot = document.getElementById('ollama-status-dot');
  const statusText = document.getElementById('ollama-status-text');
  const modelInfo = document.getElementById('ollama-model-info');
  const modelListCard = document.getElementById('model-list-card');
  const modelList = document.getElementById('model-list');
  const modelDropdown = document.getElementById('chat-model-select');

  console.log('📍 DOM elements found:', {
    statusDot: !!statusDot,
    statusText: !!statusText,
    modelInfo: !!modelInfo,
    modelListCard: !!modelListCard,
    modelList: !!modelList,
    modelDropdown: !!modelDropdown
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
      // Display model list (with safety check)
      if (modelListCard && modelList) {
        modelListCard.style.display = 'block';
        modelList.innerHTML = '';

        // Update dropdown with models
        if (modelDropdown) {
          // Keep the first option
          modelDropdown.innerHTML = '<option value="">Select a model...</option>';
          
          // Get saved model from localStorage
          const savedModel = localStorage.getItem('selectedModel');
          
          // Find the largest model by size
          let largestModel = null;
          let largestSize = 0;
          status.models.forEach(model => {
            if (model.size > largestSize) {
              largestSize = model.size;
              largestModel = model;
            }
          });

          let modelSelected = false;
          status.models.forEach(model => {
            const option = document.createElement('option');
            option.value = model.name;
            option.textContent = model.name;
            
            // Select saved model if exists, otherwise select largest model
            if (savedModel && model.name === savedModel && savedModel !== '') {
              option.selected = true;
              currentModel = model.name;
              modelSelected = true;
              console.log('🔄 Restoring saved model:', savedModel);
            } else if (!savedModel && largestModel && model.name === largestModel.name && !modelSelected) {
              option.selected = true;
              currentModel = model.name;
              modelSelected = true;
              console.log('🎯 Auto-selecting largest model:', model.name, 'Size:', formatSize(model.size));
              localStorage.setItem('selectedModel', currentModel);
            }
            
            modelDropdown.appendChild(option);
          });
          
          // Enable chat input if a model was selected
          if (modelSelected) {
            // Delay enableChatInput to ensure DOM is ready
            setTimeout(() => enableChatInput(), 100);
          }
        }

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

          modelItem.appendChild(modelInfo);
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

// Enable/disable chat input based on model selection
const enableChatInput = () => {
  console.log('🔧 enableChatInput called, currentModel:', currentModel);
  
  const input = document.getElementById('chat-input');
  const sendButton = document.getElementById('chat-send');
  const messagesContainer = document.getElementById('chat-messages');
  
  console.log('📍 DOM elements:', {
    input: !!input,
    sendButton: !!sendButton,
    messagesContainer: !!messagesContainer
  });
  
  if (!input || !sendButton) {
    console.error('❌ Chat input or send button not found!');
    return;
  }
  
  if (currentModel) {
    console.log('✅ Enabling chat input for model:', currentModel);
    input.disabled = false;
    sendButton.disabled = false;
    input.placeholder = `Type your message... (${currentModel})`;
    input.focus(); // Auto-focus the input
    
    // Clear empty state if it's the first time
    if (messagesContainer && messagesContainer.querySelector('.empty-state')) {
      messagesContainer.innerHTML = '';
      // Add system message
      chatMessages = [{
        role: 'system',
        content: 'You are a helpful assistant. Do not respond in markdown. Use plain text only.'
      }];
    }
  } else {
    console.log('⚠️ No model selected, disabling chat input');
    input.disabled = true;
    sendButton.disabled = true;
    input.placeholder = 'Select a model first...';
  }
};

// Add message to chat UI
const addMessageToUI = (role, content, isStreaming = false) => {
  const messagesContainer = document.getElementById('chat-messages');
  
  const messageDiv = document.createElement('div');
  messageDiv.className = 'chat-message';
  
  const roleDiv = document.createElement('div');
  roleDiv.className = `message-role ${role}`;
  roleDiv.textContent = role;
  
  const contentDiv = document.createElement('div');
  contentDiv.className = `message-content ${role} ${isStreaming ? 'streaming' : ''}`;
  contentDiv.textContent = content;
  
  messageDiv.appendChild(roleDiv);
  messageDiv.appendChild(contentDiv);
  messagesContainer.appendChild(messageDiv);
  
  // Auto-scroll to bottom
  messagesContainer.scrollTop = messagesContainer.scrollHeight;
  
  return contentDiv; // Return for streaming updates
};

// Send chat message
const sendChatMessage = async () => {
  const input = document.getElementById('chat-input');
  const sendButton = document.getElementById('chat-send');
  const message = input.value.trim();
  
  if (!message || !currentModel || isStreaming) return;
  
  // Add user message
  chatMessages.push({ role: 'user', content: message });
  addMessageToUI('user', message);
  
  // Clear input and disable while processing
  input.value = '';
  input.disabled = true;
  sendButton.disabled = true;
  isStreaming = true;
  
  // Add streaming assistant message
  const assistantDiv = addMessageToUI('assistant', '', true);
  let assistantContent = '';
  
  try {
    // Use the new chat method from SDK
    const chatRequest = {
      model: currentModel,
      messages: chatMessages,
      stream: true,
      options: {
        temperature: 0.7,
        num_ctx: 4096
      }
    };
    
    console.log('💬 Sending chat request:', chatRequest);
    
    // Stream the response
    for await (const chunk of Inference.chat(chatRequest)) {
      if (chunk.message && chunk.message.content) {
        assistantContent += chunk.message.content;
        assistantDiv.textContent = assistantContent;
        
        // Auto-scroll
        const messagesContainer = document.getElementById('chat-messages');
        messagesContainer.scrollTop = messagesContainer.scrollHeight;
      }
      
      if (chunk.done) {
        console.log('✅ Chat streaming complete');
        assistantDiv.classList.remove('streaming');
        break;
      }
    }
    
    // Add assistant message to history
    chatMessages.push({ role: 'assistant', content: assistantContent });
    
  } catch (error) {
    console.error('❌ Chat error:', error);
    assistantDiv.textContent = `Error: ${error.message}`;
    assistantDiv.classList.remove('streaming');
    assistantDiv.style.borderColor = 'var(--webtui-danger)';
    assistantDiv.style.color = 'var(--webtui-danger)';
  } finally {
    // Re-enable input
    isStreaming = false;
    input.disabled = false;
    sendButton.disabled = false;
    input.focus(); // Auto-focus for next message
  }
};

// Initialize chat event handlers
const initializeChatHandlers = () => {
  console.log('🎮 Initializing chat handlers...');
  
  const modelDropdown = document.getElementById('chat-model-select');
  const input = document.getElementById('chat-input');
  const sendButton = document.getElementById('chat-send');
  
  console.log('📍 Chat handler elements:', {
    modelDropdown: !!modelDropdown,
    input: !!input,
    sendButton: !!sendButton
  });
  
  if (modelDropdown) {
    modelDropdown.addEventListener('change', (e) => {
      console.log('📋 Model dropdown changed:', e.target.value);
      currentModel = e.target.value;
      localStorage.setItem('selectedModel', currentModel);
      enableChatInput();
    });
  } else {
    console.error('❌ Model dropdown not found!');
  }
  
  if (sendButton) {
    sendButton.addEventListener('click', sendChatMessage);
  } else {
    console.error('❌ Send button not found!');
  }
  
  if (input) {
    input.addEventListener('keypress', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        sendChatMessage();
      }
    });
  } else {
    console.error('❌ Chat input not found!');
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
      // Initialize chat handlers
      initializeChatHandlers();
    });
  } else {
    console.log('✅ DOM already ready, starting Ollama checks immediately');
    // DOM is already ready
    checkOllamaStatus();
    setInterval(checkOllamaStatus, 10000);
    // Initialize chat handlers
    initializeChatHandlers();
  }
};

// Initialize Ollama status checking
initializeOllamaStatus();
