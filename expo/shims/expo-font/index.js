// Web shim for 'expo-font' to avoid fontfaceobserver 6000ms timeout in WebView/Metro.
// Provides a no-op/instant-resolve implementation for web bundling.

export async function loadAsync() {
  return;
}

export function useFonts() {
  // Match real hook tuple [loaded, error]
  return [true, undefined];
}

export default { loadAsync };

