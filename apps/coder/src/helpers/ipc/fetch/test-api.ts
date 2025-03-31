// apps/coder/src/helpers/ipc/fetch/test-api.ts
// Helper function to test the local Hono API

export async function testLocalApi() {
  console.log('Testing local API connection...');
  
  try {
    if (!window.electron || typeof window.electron.fetch !== 'function') {
      console.error('electron.fetch not available - preload script may not be working correctly');
      return false;
    }
    
    const response = await window.electron.fetch('/api/ping');
    if (!response.ok) {
      throw new Error(`HTTP error: ${response.status} ${response.statusText}`);
    }
    
    const data = await response.json();
    console.log('API response:', data);
    
    if (data.message === 'pong') {
      console.log('✅ Local API is working correctly!');
      return true;
    } else {
      console.warn('⚠️ API responded but with unexpected data:', data);
      return false;
    }
  } catch (error) {
    console.error('❌ Failed to connect to local API:', error);
    return false;
  }
}