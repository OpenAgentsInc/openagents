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
  
  let updatedMessage = message;
  
  for (const { command, result } of results) {    
    const escapedCommand = command.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const tagPattern = new RegExp(
      `<execute-command>${escapedCommand}<\\/execute-command>`,
      'g'
    );
    
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
    
    updatedMessage = updatedMessage.replace(tagPattern, replacement);
  }
  
  if (updatedMessage !== message) {
    console.log(`✅ COMMAND PARSER: Replacement complete - added command results to message`);
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