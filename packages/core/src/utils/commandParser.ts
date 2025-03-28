/**
 * Parses a message for command execution tags
 * Looking for the pattern <execute-command>command</execute-command>
 */
export function parseCommandsFromMessage(message: string): string[] {
  if (message.includes("<execute-command>")) {
    console.log("🔍 COMMAND PARSER: Scanning message for command tags");
  }
  
  const commands: string[] = [];
  const commandRegex = /<execute-command>([\s\S]*?)<\/execute-command>/g;
  
  let match: RegExpExecArray | null;
  while ((match = commandRegex.exec(message)) !== null) {
    if (match[1] && match[1].trim()) {
      const command = match[1].trim();
      commands.push(command);
      console.log("✅ COMMAND PARSER: Extracted command:", command);
    }
  }
  
  if (commands.length > 0) {
    console.log(`🔢 COMMAND PARSER: Found ${commands.length} commands`);
  }
  
  return commands;
}

/**
 * Replaces command tags with execution results
 */
export function replaceCommandTagsWithResults(
  message: string,
  results: Array<{ command: string; result: string | { error: string } }>
): string {
  console.log("🔄 COMMAND PARSER: Replacing command tags with results");
  
  // Log the message and results
  console.log("🔎 COMMAND PARSER: Original message:", message);
  console.log("🎯 COMMAND PARSER: Command results:", JSON.stringify(results));
  
  let updatedMessage = message;
  
  for (const { command, result } of results) {    
    console.log(`🔍 COMMAND PARSER: Processing result for command: ${command}`);
    
    const escapedCommand = command.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const tagPattern = new RegExp(
      `<execute-command>${escapedCommand}<\\/execute-command>`,
      'g'
    );
    
    // Check if this command tag exists in the message
    const tagExists = tagPattern.test(updatedMessage);
    console.log(`🧩 COMMAND PARSER: Command tag exists in message: ${tagExists}`);
    
    // Reset the lastIndex property of the regex
    tagPattern.lastIndex = 0;
    
    let replacement: string;
    
    if (typeof result === 'string') {
      replacement = [
        '<execute-command>',
        command,
        '</execute-command>',
        '\n\n**Command Result:**\n```bash\n',
        result,
        '\n```'
      ].join('');
    } else {
      replacement = [
        '<execute-command>',
        command,
        '</execute-command>',
        '\n\n**Command Error:**\n```bash\n',
        result.error,
        '\n```'
      ].join('');
    }
    
    console.log(`🔄 COMMAND PARSER: Replacement text: ${replacement.substring(0, 50)}...`);
    
    // Save the message before replacement for comparison
    const beforeReplace = updatedMessage;
    updatedMessage = updatedMessage.replace(tagPattern, replacement);
    
    // Log if replacement occurred
    if (beforeReplace !== updatedMessage) {
      console.log(`✅ COMMAND PARSER: Command tag replaced successfully`);
    } else {
      console.log(`⚠️ COMMAND PARSER: No replacement occurred`);
    }
  }
  
  // Make one final check to ensure we have results in the final message
  const hasResults = updatedMessage.includes('**Command Result**') || 
                     updatedMessage.includes('**Command Error**');
  
  if (updatedMessage !== message) {
    console.log(`✅ COMMAND PARSER: Replacement complete - added command results to message`);
    console.log(`🔍 COMMAND PARSER: Has result section: ${hasResults}`);
  } else {
    console.log(`⚠️ COMMAND PARSER: No replacements were made to the message`);
    
    // Emergency fallback - just append all results to the end of the message
    if (!hasResults && results.length > 0) {
      console.log(`🚨 COMMAND PARSER: Using emergency fallback to append results`);
      
      updatedMessage += "\n\n";
      
      for (const { command, result } of results) {
        updatedMessage += `**Command:** \`${command}\`\n\n`;
        
        if (typeof result === 'string') {
          updatedMessage += `**Result:**\n\`\`\`bash\n${result}\n\`\`\`\n\n`;
        } else {
          updatedMessage += `**Error:**\n\`\`\`bash\n${result.error}\n\`\`\`\n\n`;
        }
      }
      
      console.log(`✅ COMMAND PARSER: Emergency fallback successful`);
    }
  }
  
  return updatedMessage;
}

/**
 * Formats the output from a command execution for display
 */
export function formatCommandOutput(
  command: string,
  result: { stdout: string; stderr: string; exitCode: number } | { error: string }
): string {
  if ('error' in result) {
    console.log(`❌ COMMAND PARSER: Command error: ${result.error}`);
    return `Error: ${result.error}`;
  }
  
  const { stdout, stderr, exitCode } = result;
  let output = '';
  
  if (stdout) {
    console.log(`📤 COMMAND PARSER: Command produced ${stdout.length} bytes of output`);
    output += stdout;
  }
  
  if (stderr) {
    if (output) output += '\n';
    output += `Error (stderr): ${stderr}`;
  }
  
  if (exitCode !== 0) {
    if (output) output += '\n';
    output += `Exit code: ${exitCode}`;
  }
  
  if (!output) {
    return 'Command executed successfully (no output)';
  }
  
  return output;
}