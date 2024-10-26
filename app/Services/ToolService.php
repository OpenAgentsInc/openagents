<?php

namespace App\Services;

use App\Models\ToolInvocation;
use App\Tools\CreateFile;
use App\Tools\ViewFile;
use App\Tools\ViewFolder;
use App\Tools\RewriteFile;
use Exception;
use Illuminate\Support\Facades\Log;

class ToolService
{
    private $toolClasses = [
        'view_file' => ViewFile::class,
        'view_folder' => ViewFolder::class,
        'rewrite_file' => RewriteFile::class,
        'create_file' => CreateFile::class,
    ];

    public function getToolDefinitions(array $toolNames = []): array
    {
        $toolDefinitions = [];

        foreach ($this->toolClasses as $toolName => $toolClass) {
            if (empty($toolNames) || in_array($toolName, $toolNames)) {
                try {
                    $definition = $toolClass::getDefinition();
                    Log::info("Retrieved definition for {$toolClass}", ['definition' => $definition]);
                    $toolDefinitions[] = $this->formatToolDefinition($definition);
                } catch (Exception $e) {
                    Log::error("Error getting definition for {$toolClass}", ['error' => $e->getMessage()]);
                }
            }
        }

        Log::info('Generated tool definitions', ['toolDefinitions' => $toolDefinitions]);
        return ['tools' => $toolDefinitions];
    }

    private function formatToolDefinition(array $definition): array
    {
        if (isset($definition['function'])) {
            $functionDef = $definition['function'];
        } else {
            $functionDef = $definition;
        }

        if (!isset($functionDef['name'])) {
            Log::error('Tool definition missing name', ['definition' => $definition]);
            throw new Exception("Tool definition must include a 'name' property");
        }

        return [
            'toolSpec' => [
                'name' => $functionDef['name'],
                'description' => $functionDef['description'] ?? '',
                'inputSchema' => [
                    'json' => $functionDef['parameters'] ?? [],
                ],
            ],
        ];
    }

    public function executeTool(string $toolName, array $args): array
    {
        Log::info('Executing tool', ['toolName' => $toolName, 'args' => $args]);
        try {
            if (!isset($this->toolClasses[$toolName])) {
                throw new Exception('Unknown tool: ' . $toolName);
            }
            $toolClass = $this->toolClasses[$toolName];
            $result = $toolClass::execute($args);
            Log::info('Tool execution completed', ['toolName' => $toolName, 'result' => $result]);
            return $result;
        } catch (Exception $e) {
            Log::error('Error executing tool', ['toolName' => $toolName, 'error' => $e->getMessage()]);
            throw $e;
        }
    }

    public function handleToolCall(array $toolCall, int $chatMessageId): array
    {
        Log::info('Handling tool call', ['toolCall' => $toolCall, 'chatMessageId' => $chatMessageId]);

        if (!isset($toolCall['toolCallId']) || !isset($toolCall['toolName']) || !isset($toolCall['args'])) {
            Log::error('Invalid tool call format', ['toolCall' => $toolCall]);
            throw new Exception('Invalid tool call format');
        }

        $invocation = new ToolInvocation([
            'tool_name' => $toolCall['toolName'],
            'input' => json_encode($toolCall['args']),
            'status' => 'pending',
            'message_id' => $chatMessageId,
        ]);
        $invocation->save();
        Log::info('Tool invocation created', ['invocation' => $invocation]);

        try {
            $result = $this->executeTool($toolCall['toolName'], $toolCall['args']);
            $invocation->output = json_encode($result);
            $invocation->status = 'completed';
        } catch (Exception $e) {
            Log::error('Error handling tool call', ['error' => $e->getMessage()]);
            $invocation->output = json_encode(['error' => $e->getMessage()]);
            $invocation->status = 'failed';
        }

        $invocation->save();
        Log::info('Tool invocation updated', ['invocation' => $invocation]);

        return [
            'type' => 'tool_call',
            'value' => [
                'toolCallId' => $toolCall['toolCallId'],
                'toolName' => $toolCall['toolName'],
                'args' => $toolCall['args'],
                'result' => $result ?? null,
            ],
        ];
    }
}
