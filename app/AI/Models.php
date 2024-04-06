<?php

namespace App\AI;

class Models
{
    public const MODELS = [
        'mistral-small-latest' => 'Mistral Small',
        'mistral-medium-latest' => 'Mistral Medium',
        'mistral-large-latest' => 'Mistral Large',
        'gpt-3.5-turbo-16k' => 'GPT-3.5 Turbo 16K',
        'gpt-4-turbo-preview' => 'GPT-4 Turbo',
        'gpt-4' => 'GPT-4',
    ];

    public static function getDefaultModel()
    {
        // If user is not logged in, use Mistral Small.
        if (! auth()->check()) {
            return 'mistral-small-latest';
        }

        // If user is logged in and is Pro, use Mistral Large.
        if (auth()->check() && auth()->user()->isPro()) {
            return 'mistral-large-latest';
        }

        // For authed non-Pro users, use Mistral Medium.
        return 'mistral-medium-latest';
    }

    public static function getModelName($model)
    {
        return self::MODELS[$model] ?? 'Unknown Model';
    }
}
