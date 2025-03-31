// apps/coder/src/helpers/ipc/fetch/test-api.ts
// Helper function to test the local Hono API

export async function testLocalApi() {
  console.log('Testing local API connection...');
  
  try {
    if (!window.electron) {
      console.error('electron API not available - preload script may not be working correctly');
      return false;
    }
    
    // First test basic IPC
    console.log('Testing basic IPC...');
    if (typeof window.electron.testIpc === 'function') {
      try {
        const result = await window.electron.testIpc();
        console.log('Basic IPC test result:', result);
        if (result.success) {
          console.log('✅ Basic IPC is working!');
        } else {
          console.warn('⚠️ Basic IPC returned unexpected result:', result);
        }
      } catch (error) {
        console.error('❌ Basic IPC test failed:', error);
      }
    } else {
      console.error('testIpc function not available - preload script may not be working correctly');
    }
    
    if (typeof window.electron.fetch !== 'function') {
      console.error('electron.fetch not available - preload script may not be working correctly');
      return false;
    }
    
    // Log the fetch function to make sure we're using the right one
    console.log('Using electron.fetch:', window.electron.fetch);
    
    // Make sure to use the electron.fetch NOT the global fetch
    // Use absolute path with localhost to avoid URL parsing issues
    const response = await window.electron.fetch('/api/ping');
    console.log('Response received:', response);
    
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