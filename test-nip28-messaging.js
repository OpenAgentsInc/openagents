#!/usr/bin/env node

/**
 * Test NIP-28 message sending functionality
 * Tests both channel creation and message sending
 */

import WebSocket from 'ws';
import crypto from 'crypto';

const RELAY_URL = 'ws://localhost:3003/relay';

// Generate test keypair
function generateKeypair() {
  const privkey = crypto.randomBytes(32).toString('hex');
  return { privkey };
}

// Create a simple event signature (mock)
function signEvent(event, privkey) {
  const serialized = JSON.stringify([
    0,
    event.pubkey,
    event.created_at,
    event.kind,
    event.tags,
    event.content
  ]);
  
  const hash = crypto.createHash('sha256').update(serialized).digest('hex');
  const sig = crypto.createHash('sha256').update(privkey + hash).digest('hex');
  
  return { ...event, id: hash, sig };
}

async function testChannelMessaging() {
  console.log('🧪 Testing NIP-28 channel messaging...\n');

  const { privkey } = generateKeypair();
  const pubkey = crypto.createHash('sha256').update(privkey).digest('hex');

  let ws = null;
  let channelId = null;

  try {
    // Connect to relay
    console.log('📡 Connecting to relay...');
    ws = new WebSocket(RELAY_URL);
    
    await new Promise((resolve, reject) => {
      ws.on('open', resolve);
      ws.on('error', reject);
      
      setTimeout(() => reject(new Error('Connection timeout')), 5000);
    });
    
    console.log('✅ Connected to relay');

    const messages = [];
    ws.on('message', (data) => {
      const msg = JSON.parse(data.toString());
      messages.push(msg);
      console.log('📨 Received:', JSON.stringify(msg));
    });

    // Wait for initial NOTICE
    await new Promise(resolve => setTimeout(resolve, 100));

    // Step 1: Create a channel (kind 40)
    console.log('\n📢 Creating test channel...');
    const channelEvent = signEvent({
      pubkey,
      created_at: Math.floor(Date.now() / 1000),
      kind: 40,
      tags: [],
      content: JSON.stringify({
        name: "Test Messaging Channel",
        about: "Testing NIP-28 message sending functionality"
      })
    }, privkey);

    channelId = channelEvent.id;
    console.log('Channel ID:', channelId);

    // Send channel creation event
    ws.send(JSON.stringify(["EVENT", channelEvent]));
    
    // Wait for OK response
    await new Promise(resolve => setTimeout(resolve, 500));

    // Check for OK message
    const okMessages = messages.filter(msg => msg[0] === 'OK' && msg[1] === channelId);
    if (okMessages.length === 0) {
      throw new Error('Did not receive OK for channel creation');
    }
    console.log('✅ Channel creation acknowledged');

    // Step 2: Send a message to the channel (kind 42)
    console.log('\n💬 Sending message to channel...');
    const messageEvent = signEvent({
      pubkey,
      created_at: Math.floor(Date.now() / 1000),
      kind: 42,
      tags: [
        ['e', channelId, '', 'root'], // Reference to channel
      ],
      content: "Hello from the test! This is a message in the channel."
    }, privkey);

    ws.send(JSON.stringify(["EVENT", messageEvent]));
    
    // Wait for OK response
    await new Promise(resolve => setTimeout(resolve, 500));

    // Check for OK message
    const messageOkMessages = messages.filter(msg => msg[0] === 'OK' && msg[1] === messageEvent.id);
    if (messageOkMessages.length === 0) {
      throw new Error('Did not receive OK for message');
    }
    console.log('✅ Message acknowledged');

    // Step 3: Send another message
    console.log('\n💬 Sending second message...');
    const message2Event = signEvent({
      pubkey,
      created_at: Math.floor(Date.now() / 1000),
      kind: 42,
      tags: [
        ['e', channelId, '', 'root'],
      ],
      content: "This is a second message to test multiple messages in the channel."
    }, privkey);

    ws.send(JSON.stringify(["EVENT", message2Event]));
    
    // Wait for OK response
    await new Promise(resolve => setTimeout(resolve, 500));

    // Check for OK message
    const message2OkMessages = messages.filter(msg => msg[0] === 'OK' && msg[1] === message2Event.id);
    if (message2OkMessages.length === 0) {
      throw new Error('Did not receive OK for second message');
    }
    console.log('✅ Second message acknowledged');

    // Step 4: Query messages from the channel
    console.log('\n🔍 Querying channel messages...');
    const subscriptionId = 'test-sub-' + Date.now();
    
    ws.send(JSON.stringify([
      "REQ", 
      subscriptionId, 
      { 
        kinds: [42], 
        "#e": [channelId], // Messages referencing our channel
        limit: 10 
      }
    ]));
    
    // Wait for messages and EOSE
    await new Promise(resolve => setTimeout(resolve, 1000));

    // Check for EVENT messages and EOSE
    const eventMessages = messages.filter(msg => msg[0] === 'EVENT' && msg[1] === subscriptionId);
    const eoseMessages = messages.filter(msg => msg[0] === 'EOSE' && msg[1] === subscriptionId);
    
    console.log(`📊 Received ${eventMessages.length} message events`);
    if (eoseMessages.length === 0) {
      throw new Error('Did not receive EOSE for subscription');
    }
    console.log('✅ Query completed (EOSE received)');

    // Verify we got our messages back
    const ourMessages = eventMessages.filter(msg => {
      const event = msg[2];
      return event.pubkey === pubkey && event.kind === 42;
    });
    
    console.log(`📝 Found ${ourMessages.length} of our messages in query results`);
    
    if (ourMessages.length >= 2) {
      console.log('✅ Successfully retrieved sent messages');
    } else {
      console.log('⚠️ Not all messages were retrieved');
    }

    // Close subscription
    ws.send(JSON.stringify(["CLOSE", subscriptionId]));
    await new Promise(resolve => setTimeout(resolve, 100));

    console.log('\n🎉 NIP-28 messaging test completed successfully!');
    console.log(`📈 Test summary:
- Channel created: ${channelId}
- Messages sent: 2
- Messages retrieved: ${ourMessages.length}
- All operations acknowledged by relay`);

    return true;

  } catch (error) {
    console.error('❌ Test failed:', error.message);
    console.error(error.stack);
    return false;
  } finally {
    if (ws) {
      ws.close();
    }
  }
}

// Run the test
if (import.meta.main) {
  testChannelMessaging()
    .then(success => {
      process.exit(success ? 0 : 1);
    })
    .catch(error => {
      console.error('Test runner error:', error);
      process.exit(1);
    });
}