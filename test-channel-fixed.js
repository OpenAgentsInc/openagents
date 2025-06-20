// Test channel creation after migration
import fetch from 'node-fetch';

async function testChannelCreation() {
  console.log('Testing channel creation after database migration...');
  
  const response = await fetch('http://localhost:3003/api/channels/create', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      name: 'Migration Test Channel',
      about: 'Testing after database migration'
    })
  });
  
  const result = await response.text();
  console.log('Response status:', response.status);
  console.log('Response:', result);
  
  try {
    const parsed = JSON.parse(result);
    console.log('Parsed response:', parsed);
    
    if (parsed.channelId) {
      console.log('✅ Channel created successfully!');
      console.log('Channel ID:', parsed.channelId);
      
      // Test listing channels
      console.log('\n📋 Testing channel listing...');
      const listResponse = await fetch('http://localhost:3003/api/channels/list');
      const listResult = await listResponse.text();
      console.log('List response status:', listResponse.status);
      console.log('List result:', listResult);
      
      const channels = JSON.parse(listResult);
      if (channels.channels && channels.channels.length > 0) {
        console.log('✅ Channel listing works!');
        console.log(`Found ${channels.channels.length} channel(s)`);
        channels.channels.forEach(ch => {
          console.log(`  - ${ch.name} (${ch.id})`);
        });
      }
    }
  } catch (e) {
    console.error('Failed to parse response:', e);
  }
}

testChannelCreation().catch(console.error);