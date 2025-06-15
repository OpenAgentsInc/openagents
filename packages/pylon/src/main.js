// Pylon - OpenAgents SDK Demo App
import { runHelloWorld, checkOllama } from '@openagentsinc/sdk';

console.log('Pylon initialized');

// Run the Effect program from SDK
runHelloWorld();

// Update Ollama status in the UI
const updateOllamaStatus = (status) => {
  const statusDot = document.getElementById('ollama-status-dot');
  const statusText = document.getElementById('ollama-status-text');
  const modelInfo = document.getElementById('ollama-model-info');
  
  // Remove all status classes
  statusDot.classList.remove('checking', 'online', 'offline');
  
  if (status.online) {
    statusDot.classList.add('online');
    statusText.textContent = 'Ollama: Online';
    
    // Show model count if available
    if (status.modelCount > 0) {
      modelInfo.style.display = 'block';
      modelInfo.querySelector('span').textContent = `${status.modelCount} model${status.modelCount !== 1 ? 's' : ''} available`;
    } else {
      modelInfo.style.display = 'none';
    }
  } else {
    statusDot.classList.add('offline');
    statusText.textContent = 'Ollama: Offline';
    modelInfo.style.display = 'none';
  }
};

// Check Ollama status on load
const checkOllamaStatus = async () => {
  const statusDot = document.getElementById('ollama-status-dot');
  statusDot.classList.add('checking');
  
  try {
    const status = await checkOllama();
    updateOllamaStatus(status);
  } catch (error) {
    console.error('Error checking Ollama status:', error);
    updateOllamaStatus({ online: false });
  }
};

// Initial check
checkOllamaStatus();

// Poll every 10 seconds
setInterval(checkOllamaStatus, 10000);