// Initial loader functionality - moved from inline script to respect CSP

// By default, disable the debug overlay unless explicitly enabled
// This prevents debug messages from showing to regular users
if (localStorage.getItem('show-debug-messages') === null) {
  localStorage.setItem('show-debug-messages', 'false');
}

// Set a timeout to detect if the app fails to load
window.loadingTimeout = setTimeout(function() {
  const loader = document.getElementById('initial-loader');
  if (loader) {
    loader.classList.add('timeout');
  }
}, 15000); // 15 seconds timeout

// Function to track loading progress
window.updateLoadingMessage = function(message) {
  const statusElement = document.getElementById('loading-status');
  if (statusElement) {
    statusElement.textContent = message;
  }
};

// Error handling during initial load
window.addEventListener('error', function(event) {
  console.error('Initial loading error:', event.error || event.message);
  const errorElement = document.getElementById('loading-error-details');
  if (errorElement) {
    errorElement.textContent = event.message || 'Unknown error during application load';
    document.getElementById('initial-loader').classList.add('timeout');
  }
});

// Script to remove loader when app is ready
window.hideInitialLoader = function() {
  const loader = document.getElementById('initial-loader');
  const app = document.getElementById('app');
  
  console.log('[Loader] Hiding initial loader, showing app');
  
  if (loader && app) {
    loader.style.display = 'none';
    app.style.display = 'block';
  }
  
  // Clear the timeout
  if (window.loadingTimeout) {
    clearTimeout(window.loadingTimeout);
  }
};

// Force show app if it takes too long (backup measure)
window.forceShowApp = setTimeout(function() {
  console.log('[Loader] Force showing app after timeout');
  
  const loader = document.getElementById('initial-loader');
  const app = document.getElementById('app');
  
  if (loader && app) {
    loader.style.display = 'none';
    app.style.display = 'block';
  }
  
  // Add a debug message to the page body for troubleshooting
  try {
    const debugEl = document.createElement('div');
    debugEl.style.position = 'fixed';
    debugEl.style.bottom = '0';
    debugEl.style.left = '0';
    debugEl.style.padding = '10px';
    debugEl.style.background = 'rgba(0,0,0,0.7)';
    debugEl.style.color = '#fff';
    debugEl.style.fontSize = '12px';
    debugEl.style.fontFamily = 'monospace';
    debugEl.style.zIndex = '99999';
    debugEl.style.display = 'flex';
    debugEl.style.alignItems = 'center';
    debugEl.style.gap = '10px';
    debugEl.style.borderRadius = '4px';
    
    // Add message
    const messageSpan = document.createElement('span');
    messageSpan.textContent = 'App was force-shown by loader - check console logs';
    
    // Add toggle debug button
    const toggleBtn = document.createElement('button');
    toggleBtn.textContent = 'Show Debug';
    toggleBtn.style.fontSize = '10px';
    toggleBtn.style.padding = '3px 6px';
    toggleBtn.style.backgroundColor = '#444';
    toggleBtn.style.border = 'none';
    toggleBtn.style.borderRadius = '3px';
    toggleBtn.style.color = 'white';
    toggleBtn.style.cursor = 'pointer';
    toggleBtn.onclick = function() {
      const isEnabled = window.toggleDebugMessages();
      this.textContent = isEnabled ? 'Hide Debug' : 'Show Debug';
    };
    
    // Add dismiss button
    const dismissBtn = document.createElement('button');
    dismissBtn.textContent = 'Dismiss';
    dismissBtn.style.fontSize = '10px';
    dismissBtn.style.padding = '3px 6px';
    dismissBtn.style.backgroundColor = '#444';
    dismissBtn.style.border = 'none';
    dismissBtn.style.borderRadius = '3px';
    dismissBtn.style.color = 'white';
    dismissBtn.style.cursor = 'pointer';
    dismissBtn.onclick = function() {
      debugEl.style.display = 'none';
    };
    
    // Add elements to container
    debugEl.appendChild(messageSpan);
    debugEl.appendChild(toggleBtn);
    debugEl.appendChild(dismissBtn);
    
    document.body.appendChild(debugEl);
  } catch (e) {
    console.error('[Loader] Error adding debug element:', e);
  }
}, 10000); // 10 seconds timeout

// Additional debug function to add logs to the page in case console is inaccessible
window.addDebugMessage = function(message) {
  try {
    // Check if debug messages should be shown (default to false)
    const showDebugMessages = localStorage.getItem('show-debug-messages') === 'true';
    
    // Always log to console regardless of visibility setting
    console.log(`[DebugLog] ${message}`);
    
    // If debug messages are disabled, don't create or update the UI
    if (!showDebugMessages) {
      return;
    }
    
    // Try to find or create a debug log container
    let debugLog = document.getElementById('debug-messages');
    if (!debugLog) {
      debugLog = document.createElement('div');
      debugLog.id = 'debug-messages';
      debugLog.style.position = 'fixed';
      debugLog.style.bottom = '10px';
      debugLog.style.right = '10px';
      debugLog.style.maxWidth = '80%';
      debugLog.style.maxHeight = '200px';
      debugLog.style.overflow = 'auto';
      debugLog.style.backgroundColor = 'rgba(0,0,0,0.8)';
      debugLog.style.color = '#fff';
      debugLog.style.fontFamily = 'monospace';
      debugLog.style.fontSize = '10px';
      debugLog.style.padding = '10px';
      debugLog.style.borderRadius = '4px';
      debugLog.style.zIndex = '999999';
      
      // Add controls for the debug panel
      const controls = document.createElement('div');
      controls.style.display = 'flex';
      controls.style.justifyContent = 'space-between';
      controls.style.marginBottom = '5px';
      controls.style.borderBottom = '1px solid rgba(255,255,255,0.2)';
      controls.style.paddingBottom = '5px';
      
      // Title
      const title = document.createElement('div');
      title.textContent = 'Debug Messages';
      title.style.fontWeight = 'bold';
      
      // Close button
      const closeBtn = document.createElement('button');
      closeBtn.textContent = '×';
      closeBtn.style.backgroundColor = 'transparent';
      closeBtn.style.border = 'none';
      closeBtn.style.color = 'white';
      closeBtn.style.cursor = 'pointer';
      closeBtn.style.fontSize = '14px';
      closeBtn.style.padding = '0 4px';
      closeBtn.onclick = function() {
        localStorage.setItem('show-debug-messages', 'false');
        debugLog.style.display = 'none';
      };
      
      controls.appendChild(title);
      controls.appendChild(closeBtn);
      debugLog.appendChild(controls);
      
      document.body.appendChild(debugLog);
    }

    // Add timestamp and message
    const messageEl = document.createElement('div');
    const time = new Date().toISOString().substring(11, 19); // HH:MM:SS
    messageEl.textContent = `[${time}] ${message}`;
    debugLog.appendChild(messageEl);

    // Scroll to bottom
    debugLog.scrollTop = debugLog.scrollHeight;
  } catch (e) {
    console.error('Error adding debug message:', e);
  }
};

// Add a global function to toggle debug messages
window.toggleDebugMessages = function() {
  const currentSetting = localStorage.getItem('show-debug-messages') === 'true';
  const newSetting = !currentSetting;
  
  localStorage.setItem('show-debug-messages', newSetting.toString());
  
  // Update visibility of existing debug log
  const debugLog = document.getElementById('debug-messages');
  if (debugLog) {
    debugLog.style.display = newSetting ? 'block' : 'none';
  } else if (newSetting) {
    // If turning on and no log exists yet, add a starter message
    window.addDebugMessage('Debug logging enabled');
  }
  
  return newSetting;
};

// Bootstrapping code
try {
  // Initialize loading message
  window.updateLoadingMessage('Initializing application...');
  
  // Add debug message (only visible if debug mode is enabled)
  window.addDebugMessage('Loader script started');
  
  // Add a note to console about debug toggle
  console.info('Debug messages can be toggled with window.toggleDebugMessages() in the console');
  
  // This will be executed after DOM is loaded
  document.addEventListener('DOMContentLoaded', function() {
    console.log('[Bootstrap] DOM loaded, importing renderer');
    window.addDebugMessage('DOM loaded, preparing to load renderer');
    
    // Check and report page state
    try {
      const initialLoader = document.getElementById('initial-loader');
      const appElement = document.getElementById('app');
      
      window.addDebugMessage(`Initial loader exists: ${Boolean(initialLoader)}`);
      window.addDebugMessage(`App element exists: ${Boolean(appElement)}`);
      
      if (appElement) {
        window.addDebugMessage(`App display style: ${appElement.style.display || 'not set'}`);
      }
    } catch (e) {
      window.addDebugMessage(`Error checking page elements: ${e.message}`);
    }
    
    // Create a script element to load the renderer
    // This approach is more compatible with strict CSP
    const rendererScript = document.createElement('script');
    rendererScript.type = 'module';
    rendererScript.src = '/src/renderer.ts';
    
    rendererScript.onload = function() {
      window.addDebugMessage('Renderer script loaded successfully');
    };
    
    rendererScript.onerror = function(error) {
      console.error('[Bootstrap] Failed to load renderer script:', error);
      window.addDebugMessage(`Error loading renderer: ${error}`);
      
      const errorElement = document.getElementById('loading-error-details');
      if (errorElement) {
        errorElement.textContent = `Error loading application: Script load failed`;
        document.getElementById('initial-loader').classList.add('timeout');
      }
      
      // Show app element in case of error
      const appEl = document.getElementById('app');
      if (appEl) {
        appEl.style.display = 'block';
      }
    };
    
    document.body.appendChild(rendererScript);
    console.log('[Bootstrap] Renderer script added to DOM');
    window.addDebugMessage('Renderer script tag added to DOM');
  });
} catch (error) {
  console.error('[Bootstrap] Critical bootstrapping error:', error);
}