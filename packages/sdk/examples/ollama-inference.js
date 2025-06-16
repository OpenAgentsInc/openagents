#!/usr/bin/env node

// Example: Using OpenAgents SDK with Ollama for AI Inference
// This demonstrates all the new Ollama inference capabilities

import { Inference } from '@openagentsinc/sdk';

console.log('🚀 OpenAgents SDK - Ollama Inference Example');
console.log('='.repeat(60));

async function main() {
  try {
    // 1. List available models
    console.log('\n📋 Listing available Ollama models...');
    const models = await Inference.listModels();
    
    if (models.length === 0) {
      console.log('❌ No models found. Is Ollama running?');
      console.log('💡 Start Ollama with: ollama serve');
      console.log('💡 Pull a model with: ollama pull llama3.2');
      return;
    }
    
    console.log(`✅ Found ${models.length} models:`);
    models.forEach(model => {
      console.log(`   - ${model.id} (created: ${new Date(model.created * 1000).toLocaleDateString()})`);
    });
    
    // 2. Basic inference
    console.log('\n🧠 Performing basic inference...');
    const basicResponse = await Inference.infer({
      system: "You are a helpful assistant focused on Bitcoin and digital agents.",
      messages: [
        { role: "user", content: "What is the Lightning Network in one sentence?" }
      ],
      max_tokens: 100,
      temperature: 0.7,
      model: models[0]?.id || "llama3.2"
    });
    
    console.log('✅ Basic inference result:');
    console.log(`   Model: ${basicResponse.model}`);
    console.log(`   Response: ${basicResponse.content}`);
    console.log(`   Tokens: ${basicResponse.usage.total_tokens}`);
    console.log(`   Latency: ${basicResponse.latency}ms`);
    console.log(`   Finish reason: ${basicResponse.finish_reason}`);
    
    // 3. JSON mode inference
    console.log('\n📊 Testing JSON mode...');
    const jsonResponse = await Inference.infer({
      system: "You are a helpful assistant that always responds in valid JSON format.",
      messages: [
        { role: "user", content: "List 3 benefits of using Lightning Network for micropayments" }
      ],
      max_tokens: 200,
      response_format: { type: "json_object" },
      model: models[0]?.id || "llama3.2"
    });
    
    console.log('✅ JSON mode result:');
    try {
      const parsed = JSON.parse(jsonResponse.content);
      console.log('   Valid JSON:', JSON.stringify(parsed, null, 2));
    } catch (e) {
      console.log('   Raw response:', jsonResponse.content);
    }
    
    // 4. Streaming inference
    console.log('\n🌊 Testing streaming inference...');
    console.log('   Streaming response: ');
    process.stdout.write('   ');
    
    let streamedContent = '';
    for await (const chunk of Inference.inferStream({
      system: "You are a concise assistant.",
      messages: [
        { role: "user", content: "Explain digital agent economics in 2 sentences." }
      ],
      max_tokens: 100,
      model: models[0]?.id || "llama3.2"
    })) {
      process.stdout.write(chunk.content);
      streamedContent += chunk.content;
      
      if (chunk.finish_reason) {
        console.log(`\n   [Finished: ${chunk.finish_reason}]`);
      }
    }
    
    // 5. Multi-turn conversation
    console.log('\n💬 Testing multi-turn conversation...');
    const messages = [
      { role: "user", content: "What is a satoshi?" },
      { role: "assistant", content: "A satoshi is the smallest unit of Bitcoin, equal to 0.00000001 BTC." },
      { role: "user", content: "How many satoshis are in a Bitcoin?" }
    ];
    
    const conversationResponse = await Inference.infer({
      system: "You are a Bitcoin educator.",
      messages,
      max_tokens: 100,
      model: models[0]?.id || "llama3.2"
    });
    
    console.log('✅ Conversation response:', conversationResponse.content);
    
    // 6. Generate embeddings (if supported)
    console.log('\n🔢 Testing embeddings generation...');
    try {
      const embeddingModel = models.find(m => m.id.includes('embed')) || models[0];
      const embeddings = await Inference.embeddings({
        model: embeddingModel.id,
        input: "Bitcoin is digital gold"
      });
      
      console.log('✅ Embeddings generated:');
      console.log(`   Model: ${embeddings.model}`);
      console.log(`   Dimensions: ${embeddings.embeddings[0]?.length || 0}`);
      console.log(`   Tokens used: ${embeddings.usage.total_tokens}`);
    } catch (error) {
      console.log('❌ Embeddings not supported or failed:', error.message);
    }
    
    // 7. Test with different parameters
    console.log('\n🎛️ Testing different generation parameters...');
    const paramTest = await Inference.infer({
      system: "You are a creative writer.",
      messages: [
        { role: "user", content: "Write a haiku about Bitcoin" }
      ],
      max_tokens: 100,
      temperature: 1.2,  // Higher temperature for creativity
      top_p: 0.9,
      seed: 42,  // For reproducibility
      model: models[0]?.id || "llama3.2"
    });
    
    console.log('✅ Creative response with high temperature:');
    console.log(`   ${paramTest.content}`);
    
    // 8. Error handling demonstration
    console.log('\n🛡️ Testing error handling...');
    try {
      await Inference.infer({
        system: "Test",
        messages: [{ role: "user", content: "Test" }],
        max_tokens: 10,
        model: "non-existent-model"
      });
    } catch (error) {
      console.log('✅ Error handled correctly:', error.message);
    }
    
  } catch (error) {
    console.error('❌ Example failed:', error);
  }
}

// Run the example
main().then(() => {
  console.log('\n' + '='.repeat(60));
  console.log('✅ Ollama inference example completed!');
  console.log('💡 Try different models by running: ollama pull <model-name>');
  console.log('='.repeat(60));
});