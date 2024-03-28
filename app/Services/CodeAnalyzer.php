<?php

namespace App\Services;

class CodeAnalyzer
{
    public static function generatePrompt(array $filepaths): string
    {
        $prompt = '';

        foreach ($filepaths as $filepath) {
            $prompt .= "### File: {$filepath}\n\n```".
                file_get_contents($filepath)."```\n\n";
        }

        return $prompt;
    }
}
