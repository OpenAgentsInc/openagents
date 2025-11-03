// Web shim for 'expo-font' to avoid fontfaceobserver 6000ms timeout in WebView/Metro.
// Provides a no-op/instant-resolve implementation for web bundling.

const _loaded = new Set();
const _loading = new Set();

export async function loadAsync(nameOrMap, maybeSource) {
  // Support loadAsync({ Family: source, ... }) and loadAsync('Family', source)
  if (nameOrMap && typeof nameOrMap === 'object' && !Array.isArray(nameOrMap)) {
    for (const k of Object.keys(nameOrMap)) {
      _loaded.add(k);
      _loading.delete(k);
    }
    return;
  }
  if (typeof nameOrMap === 'string') {
    const k = nameOrMap;
    _loading.add(k);
    // Immediately mark as loaded in shim
    _loading.delete(k);
    _loaded.add(k);
    return;
  }
}

export function isLoaded(name) {
  return _loaded.has(name);
}

export function isLoading(name) {
  return _loading.has(name);
}

export async function unloadAsync(name) {
  _loaded.delete(name);
  _loading.delete(name);
}

export function processFontFamily(family) {
  return family;
}

export function useFonts() {
  // Match real hook tuple [loaded, error]
  return [true, undefined];
}

export function resetServerContext() {
  // SSR-side hook in expo-font; noop in our web shim
}

export function getServerResources() {
  // expo-router injects these <link> elements into <head> during SSR.
  // We already inject @font-face via CSS elsewhere, so return an empty set.
  return [];
}

export default { loadAsync, resetServerContext, getServerResources };
