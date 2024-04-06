<?php

namespace App\AI;

class Models
{
    public const MODELS = [
        'mistral-small-latest' => [
            'name' => 'Mistral Small',
            'gateway' => 'mistral',
            'access' => 'guest',
            'max_tokens' => 2000,
        ],
        'mistral-medium-latest' => [
            'name' => 'Mistral Medium',
            'gateway' => 'mistral',
            'access' => 'user',
            'max_tokens' => 2000,
        ],
        'mistral-large-latest' => [
            'name' => 'Mistral Large',
            'gateway' => 'mistral',
            'access' => 'pro',
            'max_tokens' => 4096,
        ],
        'gpt-3.5-turbo-16k' => [
            'name' => 'GPT-3.5 Turbo 16K',
            'gateway' => 'openai',
            'access' => 'user',
            'max_tokens' => 14000,
        ],
        'gpt-4-turbo-preview' => [
            'name' => 'GPT-4 Turbo',
            'gateway' => 'openai',
            'access' => 'user',
            'max_tokens' => 2000,
        ],
        'gpt-4' => [
            'name' => 'GPT-4',
            'gateway' => 'openai',
            'access' => 'pro',
            'max_tokens' => 4096,
        ],
        'claude-3-haiku-20240307' => [
            'name' => 'Claude Haiku',
            'gateway' => 'anthropic',
            'access' => 'guest',
            'max_tokens' => 4096,
        ],
        'claude-3-sonnet-20240229' => [
            'name' => 'Claude Sonnet',
            'gateway' => 'anthropic',
            'access' => 'user',
            'max_tokens' => 4096,
        ],
        'claude-3-opus-20240229' => [
            'name' => 'Claude Opus',
            'gateway' => 'anthropic',
            'access' => 'pro',
            'max_tokens' => 4096,
        ],
    ];

    public static function getDefaultModel()
    {
        // If user is not logged in, use Mistral Small.
        if (! auth()->check()) {
            return 'mistral-small-latest';
        }

        // If user is logged in and is Pro, use Mistral Large.
        if (auth()->check() && auth()->user()->isPro()) {
            return 'claude-3-sonnet-20240229';
        }

        // For authed non-Pro users, use Mistral Medium.
        return 'mistral-medium-latest';
    }

    public static function getModelName($model)
    {
        return self::MODELS[$model]['name'] ?? 'Unknown Model';
    }
}
