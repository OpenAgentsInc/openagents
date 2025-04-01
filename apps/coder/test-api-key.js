// Test if the OpenRouter API key works correctly
const fs = require('fs');
const path = require('path');
const dotenv = require('dotenv');

// Load environment variables
dotenv.config();
console.log('Loading environment variables...');

// Try to read API key from .env file as fallback
let OPENROUTER_API_KEY = process.env.OPENROUTER_API_KEY || "";
if (!OPENROUTER_API_KEY) {
  try {
    const envPath = path.join(process.cwd(), '.env');
    if (fs.existsSync(envPath)) {
      const envContent = fs.readFileSync(envPath, 'utf8');
      const match = envContent.match(/OPENROUTER_API_KEY=(.+)/);
      if (match && match[1]) {
        OPENROUTER_API_KEY = match[1].trim();
      }
    }
  } catch (error) {
    console.error("Error reading .env file directly:", error);
  }
}

console.log(`API Key found: ${OPENROUTER_API_KEY ? OPENROUTER_API_KEY.substring(0, 5) + '...' : 'None'}`);

// Simple test function
async function testOpenRouterAPIKey() {
  const url = 'https://openrouter.ai/api/v1/chat/completions';
  
  const headers = {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${OPENROUTER_API_KEY}`,
    'HTTP-Referer': 'http://localhost:3001',
    'X-Title': 'API Key Test'
  };
  
  const data = {
    model: "anthropic/claude-3-haiku-20240307",
    messages: [
      { role: "user", content: "Say hello!" }
    ],
    max_tokens: 10
  };
  
  try {
    console.log('Testing OpenRouter API key with a simple request...');
    const response = await fetch(url, {
      method: 'POST',
      headers: headers,
      body: JSON.stringify(data)
    });
    
    const result = await response.json();
    
    if (response.ok) {
      console.log('✅ API key is valid! Got response:');
      console.log(JSON.stringify(result, null, 2));
    } else {
      console.error('❌ API key test failed with status:', response.status);
      console.error('Error details:', result);
    }
  } catch (error) {
    console.error('❌ Request failed:', error);
  }
}

// Run the test
testOpenRouterAPIKey();