const { getDefaultConfig } = require('@expo/metro-config');

const config = getDefaultConfig(__dirname);

// Disable dev server logs and banners
config.server = {
  ...config.server,
  enhanceMiddleware: (middleware) => {
    return (req, res, next) => {
      // Suppress dev server logs
      const originalWrite = res.write;
      const originalEnd = res.end;
      
      res.write = function(...args) {
        const data = args[0];
        if (typeof data === 'string' && data.includes('Welcome to Metro')) {
          return;
        }
        return originalWrite.apply(this, args);
      };
      
      res.end = function(...args) {
        const data = args[0];
        if (typeof data === 'string' && data.includes('Welcome to Metro')) {
          return;
        }
        return originalEnd.apply(this, args);
      };
      
      return middleware(req, res, next);
    };
  }
};

module.exports = config;