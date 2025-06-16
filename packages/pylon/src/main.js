// Pylon - OpenAgents SDK Demo App
import { runHelloWorld, checkOllama } from '@openagentsinc/sdk';

console.log('Pylon initialized');

// Run the Effect program from SDK
runHelloWorld();

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
