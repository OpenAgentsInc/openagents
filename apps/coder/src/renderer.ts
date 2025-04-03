import { logger } from "@openagents/core";

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
    }
  });
}

import './entry';