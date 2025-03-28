/**
 * Parses a message for command execution tags
 * Looking for the pattern <execute-command>command</execute-command>
 */
export function parseCommandsFromMessage(message: string): string[] {
  const commands: string[] = [];
  const commandRegex = /<execute-command>([\s\S]*?)<\/execute-command>/g;
  
  let match: RegExpExecArray | null;
  while ((match = commandRegex.exec(message)) !== null) {
    if (match[1] && match[1].trim()) {
      commands.push(match[1].trim());
    }
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
        '\n\n```bash-output\n',
        result,
        '\n```'
      ].join('');
    } else {
      replacement = [
        '<execute-command>',
        command,
        '</execute-command>',
        '\n\n```bash-error\n',
        result.error,
        '\n```'
      ].join('');
    }
    
    updatedMessage = updatedMessage.replace(tagPattern, replacement);
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
    return `Error: ${result.error}`;
  }
  
  const { stdout, stderr, exitCode } = result;
  let output = '';
  
  if (stdout) {
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
  
  return output || 'Command executed successfully (no output)';
}