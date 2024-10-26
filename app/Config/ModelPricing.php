<?php

namespace App\Config;

class ModelPricing
{
    public static $defaultChatModel = 'anthropic.claude-3-5-sonnet-20240620-v1:0';

    public static $models = [
        [
            'name' => 'Llama 3.1 8B Instant 128k',
            'id' => 'llama-3.1-8b-instant',
            'provider' => 'groq',
            'providerCentsPerMillionInputTokens' => 5,
            'providerCentsPerMillionOutputTokens' => 8
        ],
        [
            'name' => 'Llama 3 Groq 70B Tool Use Preview 8K',
            'id' => 'llama3-groq-70b-8192-tool-use-preview',
            'provider' => 'groq',
            'providerCentsPerMillionInputTokens' => 89,
            'providerCentsPerMillionOutputTokens' => 89
        ],
        [
            'name' => 'Claude 3.5 Sonnet',
            'id' => 'anthropic.claude-3-5-sonnet-20240620-v1:0',
            'provider' => 'bedrock',
            'providerCentsPerMillionInputTokens' => 300,
            'providerCentsPerMillionOutputTokens' => 1500
        ],
        [
            'name' => 'Claude 3 Haiku',
            'id' => 'anthropic.claude-3-haiku-20240307-v1:0',
            'provider' => 'bedrock',
            'providerCentsPerMillionInputTokens' => 25,
            'providerCentsPerMillionOutputTokens' => 125
        ],
    ];

    public static function getPriceById($id)
    {
        foreach (self::$models as $model) {
            if ($model['id'] === $id) {
                return [
                    'input' => $model['providerCentsPerMillionInputTokens'],
                    'output' => $model['providerCentsPerMillionOutputTokens']
                ];
            }
        }
        throw new \Exception("Model with ID {$id} not found");
    }
}
