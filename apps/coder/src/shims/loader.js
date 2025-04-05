// Initial loader functionality - moved from inline script to respect CSP

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
  
  if (loader && app) {
    loader.style.display = 'none';
    app.style.display = 'block';
  }
  
  // Clear the timeout
  if (window.loadingTimeout) {
    clearTimeout(window.loadingTimeout);
  }
};

// Bootstrapping code
try {
  // Initialize loading message
  window.updateLoadingMessage('Initializing application...');
  
  // This will be executed after DOM is loaded
  document.addEventListener('DOMContentLoaded', function() {
    console.log('[Bootstrap] DOM loaded, importing renderer');
    
      // Create a script element to load the renderer
    // This approach is more compatible with strict CSP
    const rendererScript = document.createElement('script');
    rendererScript.type = 'module';
    rendererScript.src = '/src/renderer.ts';
    rendererScript.onerror = function(error) {
      console.error('[Bootstrap] Failed to load renderer script:', error);
      const errorElement = document.getElementById('loading-error-details');
      if (errorElement) {
        errorElement.textContent = `Error loading application: Script load failed`;
        document.getElementById('initial-loader').classList.add('timeout');
      }
    };
    document.body.appendChild(rendererScript);
    console.log('[Bootstrap] Renderer script added to DOM');
  });
} catch (error) {
  console.error('[Bootstrap] Critical bootstrapping error:', error);
}