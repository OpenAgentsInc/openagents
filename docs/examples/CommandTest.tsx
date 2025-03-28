import React, { useState } from 'react';
import { useChat } from '@openagents/core';

export const CommandTest: React.FC = () => {
  const [input, setInput] = useState('');
  const { messages, append, isLoading } = useChat({
    // This will work in both web and Electron environments
    api: 'https://chat.openagents.com',
    onError: (error) => {
      console.error('Chat error:', error);
    }
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim()) return;
    
    append({
      content: input,
      role: 'user'
    });
    setInput('');
  };

  return (
    <div style={{ maxWidth: '800px', margin: '0 auto', padding: '20px' }}>
      <h1>Command Execution Test</h1>
      <p>
        Try these examples:
      </p>
      <ul>
        <li><code>Hello, how are you?</code> - Basic chat</li>
        <li><code>Run this command: &lt;execute-command&gt;echo "Hello World"&lt;/execute-command&gt;</code> - Command execution test</li>
        <li><code>Run these commands: &lt;execute-command&gt;ls -la&lt;/execute-command&gt; and &lt;execute-command&gt;pwd&lt;/execute-command&gt;</code> - Multiple commands</li>
      </ul>
      
      <div style={{ 
        border: '1px solid #ccc', 
        borderRadius: '4px', 
        height: '400px', 
        overflow: 'auto',
        padding: '10px',
        marginBottom: '20px',
        backgroundColor: '#f5f5f5'
      }}>
        {messages.map((message) => (
          <div 
            key={message.id} 
            style={{ 
              margin: '10px 0', 
              padding: '10px', 
              borderRadius: '4px',
              backgroundColor: message.role === 'user' ? '#dcf8c6' : '#fff',
              alignSelf: message.role === 'user' ? 'flex-end' : 'flex-start',
              maxWidth: '80%',
              boxShadow: '0 1px 2px rgba(0,0,0,0.1)'
            }}
          >
            <div style={{ fontWeight: 'bold', marginBottom: '5px' }}>
              {message.role === 'user' ? 'You' : 'Assistant'}:
            </div>
            <div style={{ whiteSpace: 'pre-wrap' }}>
              {message.content}
            </div>
          </div>
        ))}
        {isLoading && (
          <div style={{ textAlign: 'center', padding: '10px' }}>
            Thinking...
          </div>
        )}
      </div>
      
      <form onSubmit={handleSubmit} style={{ display: 'flex' }}>
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Type a message..."
          style={{ 
            flex: 1, 
            padding: '10px', 
            borderRadius: '4px 0 0 4px',
            border: '1px solid #ccc'
          }}
        />
        <button 
          type="submit"
          disabled={isLoading || !input.trim()}
          style={{ 
            padding: '10px 20px', 
            backgroundColor: '#4CAF50',
            color: 'white',
            border: 'none',
            borderRadius: '0 4px 4px 0',
            cursor: isLoading || !input.trim() ? 'not-allowed' : 'pointer'
          }}
        >
          Send
        </button>
      </form>
      
      <div style={{ marginTop: '20px', padding: '10px', backgroundColor: '#fffde7', borderRadius: '4px' }}>
        <h3>Environment Detection:</h3>
        <p>Running in: <strong>{typeof window !== 'undefined' ? 'Browser' : 'Node.js'}</strong></p>
        <p>Child Process available: <strong>{typeof window !== 'undefined' && window.require ? 'Yes' : 'No'}</strong></p>
        <p>This helps verify that our environment detection is working correctly.</p>
      </div>
    </div>
  );
};