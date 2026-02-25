<?php

namespace Laravel\Ai\Messages;

enum MessageRole: string
{
    case Assistant = 'assistant';
    case User = 'user';
    case ToolResult = 'tool_result';
}
