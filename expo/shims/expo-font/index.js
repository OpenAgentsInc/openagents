// Web shim for 'expo-font' to avoid fontfaceobserver 6000ms timeout in WebView/Metro.
// Provides a no-op/instant-resolve implementation for web bundling.

export async function loadAsync() {
  return;
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
