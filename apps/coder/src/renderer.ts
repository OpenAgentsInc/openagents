/**
 * Renderer main entry point
 * This file is executed before any React code and can be used for early setup
 */

import { logger } from "@openagents/core";

// Initialize error handling
function setupErrorHandling() {
  // Create global error handlers
  window.onerror = (message, source, lineno, colno, error) => {
    console.error('[FATAL ERROR]', { message, source, lineno, colno, error });
    
    // Attempt to show a visible error
    try {
      document.body.innerHTML = `
        <div style="padding: 20px; font-family: monospace; background-color: #300; color: white; margin: 20px; border-radius: 4px;">
          <h1 style="margin: 0 0 10px 0;">Fatal Error</h1>
          <p>The application encountered a fatal error:</p>
          <div style="background-color: #200; padding: 10px; border-radius: 4px; margin-top: 10px;">
            <p style="margin: 0;"><strong>${message}</strong></p>
            <p style="margin: 5px 0 0 0;">Location: ${source}:${lineno}:${colno}</p>
            ${error?.stack ? `<pre style="margin: 10px 0 0 0; white-space: pre-wrap;">${error.stack}</pre>` : ''}
          </div>
          <button onclick="window.location.reload()" style="margin-top: 15px; padding: 8px 12px; background-color: #822; color: white; border: none; border-radius: 4px; cursor: pointer;">
            Reload Application
          </button>
        </div>
      `;
    } catch (displayError) {
      console.error('Failed to display error UI:', displayError);
    }
    
    return false; // Let default error handling continue
  };
}

// Initialize logging setup
function setupLogging() {
  // Initialize base logging without console interception
  // Console interception will be enabled by the user through the debug page
  if (process.env.NODE_ENV === 'production') {
    logger.info('Application started in production mode');
    
    // Add some detailed startup logging
    logger.info('Initializing renderer process');
    
    // Log important environment information for debugging
    logger.info('Environment information', {
      userAgent: navigator.userAgent,
      platform: navigator.platform,
      language: navigator.language,
      screenSize: {
        width: window.screen.width,
        height: window.screen.height
      },
      viteEnv: {
        mode: import.meta.env.MODE,
        dev: import.meta.env.DEV,
        prod: import.meta.env.PROD
      }
    });
  } else {
    // Development mode - add more verbose logging
    console.log('[Renderer] Starting in development mode');
    console.log('[Renderer] Environment:', import.meta.env);
  }

  // Setup performance monitoring for production
  if (process.env.NODE_ENV === 'production') {
    // Track initial load performance
    window.addEventListener('load', () => {
      if (window.performance) {
        const timing = window.performance.timing;
        const navigationStart = timing.navigationStart;
        
        const timingMetrics = {
          total: timing.loadEventEnd - navigationStart,
          network: timing.responseEnd - timing.fetchStart,
          domProcessing: timing.domComplete - timing.domLoading,
          rendering: timing.loadEventEnd - timing.domContentLoadedEventEnd
        };
        
        logger.info('Page load performance metrics', timingMetrics);
      }
    });
  }
}

// Apply initial setup
try {
  console.log('[Renderer] Starting renderer initialization...');
  setupErrorHandling();
  setupLogging();
  console.log('[Renderer] Renderer initialization complete, loading app...');
} catch (error) {
  console.error('[Renderer] Critical error during renderer initialization:', error);
}

// Import the entry point that will start React
import './entry';