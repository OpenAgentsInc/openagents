import React, { useState, useEffect } from 'react';
import { safeExecuteCommand } from '@openagents/core/src/utils/commandExecutor';

export const SimpleCommandTest: React.FC = () => {
  const [command, setCommand] = useState('echo "Hello World"');
  const [result, setResult] = useState<{success: boolean, output: string}>({
    success: false,
    output: 'No command executed yet'
  });
  const [isExecuting, setIsExecuting] = useState(false);
  const [environment, setEnvironment] = useState({
    isNode: false,
    hasChildProcess: false
  });

  // Check environment on component mount
  useEffect(() => {
    setEnvironment({
      isNode: typeof process !== 'undefined' && process.versions && process.versions.node,
      hasChildProcess: typeof require !== 'undefined' && Boolean(require('child_process'))
    });
  }, []);

  const handleExecute = async () => {
    if (!command.trim()) return;
    
    setIsExecuting(true);
    try {
      const commandResult = await safeExecuteCommand(command);
      
      if ('error' in commandResult) {
        setResult({
          success: false,
          output: `Error: ${commandResult.error}`
        });
      } else {
        setResult({
          success: true,
          output: `Command: ${command}\n\nStdout:\n${commandResult.stdout || '(empty)'}\n\nStderr:\n${commandResult.stderr || '(empty)'}\n\nExit code: ${commandResult.exitCode}`
        });
      }
    } catch (error) {
      setResult({
        success: false,
        output: `Exception: ${error instanceof Error ? error.message : String(error)}`
      });
    } finally {
      setIsExecuting(false);
    }
  };

  return (
    <div style={{ maxWidth: '800px', margin: '0 auto', padding: '20px' }}>
      <h1>Simple Command Execution Test</h1>
      <p>This test directly calls the command execution functionality, bypassing the chat interface.</p>
      
      <div style={{ marginBottom: '20px' }}>
        <label style={{ display: 'block', marginBottom: '8px', fontWeight: 'bold' }}>
          Command:
        </label>
        <div style={{ display: 'flex' }}>
          <input
            type="text"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            placeholder="Enter a command..."
            style={{ 
              flex: 1, 
              padding: '10px', 
              borderRadius: '4px 0 0 4px',
              border: '1px solid #ccc'
            }}
          />
          <button 
            onClick={handleExecute}
            disabled={isExecuting || !command.trim()}
            style={{ 
              padding: '10px 20px', 
              backgroundColor: '#4CAF50',
              color: 'white',
              border: 'none',
              borderRadius: '0 4px 4px 0',
              cursor: isExecuting || !command.trim() ? 'not-allowed' : 'pointer'
            }}
          >
            {isExecuting ? 'Executing...' : 'Execute'}
          </button>
        </div>
      </div>
      
      <div style={{ marginBottom: '20px' }}>
        <h3>Example commands to try:</h3>
        <ul>
          <li><code>echo "Hello World"</code> - Simple echo command</li>
          <li><code>ls -la</code> - List directory contents</li>
          <li><code>pwd</code> - Print working directory</li>
          <li><code>whoami</code> - Show current user</li>
          <li><code>date</code> - Current date and time</li>
        </ul>
      </div>
      
      <div style={{ 
        border: '1px solid #ccc', 
        borderRadius: '4px', 
        padding: '10px',
        backgroundColor: result.success ? '#f0f9f0' : '#fff0f0',
        minHeight: '200px',
        maxHeight: '400px',
        overflow: 'auto',
        fontFamily: 'monospace',
        whiteSpace: 'pre-wrap'
      }}>
        <h3>Result:</h3>
        {result.output}
      </div>
      
      <div style={{ marginTop: '20px', padding: '10px', backgroundColor: '#fffde7', borderRadius: '4px' }}>
        <h3>Environment Detection:</h3>
        <p>Running in: <strong>{environment.isNode ? 'Node.js' : 'Browser'}</strong></p>
        <p>Child Process available: <strong>{environment.hasChildProcess ? 'Yes' : 'No'}</strong></p>
        <p>This helps verify that our environment detection is working correctly.</p>
      </div>
    </div>
  );
};