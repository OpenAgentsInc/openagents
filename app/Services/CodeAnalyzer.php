<?php

namespace App\Services;

class CodeAnalyzer
{
    public static function generatePrompt(array $filepaths): string
    {
        $prompt = '';

        foreach ($filepaths as $filepath) {
            $prompt .= file_get_contents($filepath);
        }

        return $prompt;
    }
}
