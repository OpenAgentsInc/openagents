// Force Metro to use the CommonJS variant of Zustand middleware
// to avoid `import.meta.env` in ESM bundles triggering parse errors
module.exports = require('zustand/middleware.js')

