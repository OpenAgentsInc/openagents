<?php

namespace App\Services;

use Illuminate\Support\Facades\Process;
use Illuminate\Support\Facades\Log;

class ShellCommandService
{
    public function executeCommand($command)
    {
        Log::info('Executing shell command', ['command' => $command]);

        // Whitelist of allowed commands
        $allowedCommands = ['ls', 'pwd', 'echo', 'date'];

        // Check if the command is allowed
        if (!$this->isCommandAllowed($command, $allowedCommands)) {
            return "Error: Command not allowed";
        }

        // Execute the command
        $result = Process::run($command);

        if ($result->successful()) {
            return $result->output();
        } else {
            return "Error: " . $result->errorOutput();
        }
    }

    private function isCommandAllowed($command, $allowedCommands)
    {
        $commandParts = explode(' ', $command);
        return in_array($commandParts[0], $allowedCommands);
    }
}
