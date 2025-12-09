/**
 * Effuse HMR Module
 *
 * Hot Module Replacement support for Effuse components.
 */

export {
  saveComponentState,
  loadComponentState,
  hasComponentState,
  clearAllState,
  getHMRVersion,
  bumpHMRVersion,
} from "./registry.js"
