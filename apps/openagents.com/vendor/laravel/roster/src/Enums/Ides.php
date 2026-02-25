<?php

namespace Laravel\Roster\Enums;

enum Ides: string
{
    case PHPSTORM = 'phpstorm';
    case CURSOR = 'cursor';
    case WINDSURF = 'windsurf';
    case VSCODE = 'vscode';
    case CLAUDE_CODE = 'claudecode';
    case CODEX = 'codex';
    case OPENCODE = 'opencode';
}
