<?php

namespace App\Services;

use Illuminate\Support\Facades\Process;
use Illuminate\Support\Facades\Log;

class ShellCommandService
{
public function executeShellCommand($command)
{
    return [
        'type' => 'shell_command',
        'content' => $command
    ];
}

    private function isCommandAllowed($command, $allowedCommands)
    {
        $commandParts = explode(' ', $command);
        return in_array($commandParts[0], $allowedCommands);
    }
}
