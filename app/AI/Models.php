<?php

namespace App\AI;

use Illuminate\Support\Facades\Auth;

class Models
{
    public const array MODELS = [
        // LLAMA
        'meta-llama/llama-3-8b-chat-hf' => [
            'name' => 'Llama 3 8B',
            'gateway' => 'meta',
            'access' => 'user',
            'max_tokens' => 4096,
            'description' => 'Llama 3 8B is a language model by Meta,
                             optimized for efficient deployment on consumer-size GPUs,
                             and excels in various natural language processing tasks.',
        ],

        'meta-llama/llama-3-70b-chat-hf' => [
            'name' => 'Llama 3 70B',
            'gateway' => 'meta',
            'access' => 'pro',
            'max_tokens' => 4096,
            'description' => 'Llama 3 70B is a larger language model by Meta,
                              designed for complex tasks, and excels in various natural language processing tasks.',
        ],

        // GROQ
        'llama3-8b-8192' => [
            'name' => 'Llama 3 8B 8192',
            'gateway' => 'groq',
            'access' => 'user',
            'max_tokens' => 8192,
            'description' => 'Llama 3 8B is a language model by Meta,
                             optimized for efficient deployment on consumer-size GPUs,
                             and excels in various natural language processing tasks.',
        ],

        'llama3-70b-8192' => [
            'name' => 'Llama 3 70B 8192',
            'gateway' => 'groq',
            'access' => 'pro',
            'max_tokens' => 8192,
            'description' => 'Llama 3 70B is a larger language model by Meta,
                              designed for complex tasks, and excels in various natural language processing tasks.',
        ],

        // MISTRAL
        'mistral-small-latest' => [
            'name' => 'Mistral Small',
            'gateway' => 'mistral',
            'access' => 'guest',
            'max_tokens' => 2000,
            'description' => 'Mistral-Small is a balanced, efficient language model offering high
                                 performance across various tasks with lower latency',
        ],
        'mistral-medium-latest' => [
            'name' => 'Mistral Medium',
            'gateway' => 'mistral',
            'access' => 'user',
            'max_tokens' => 2000,
            'description' => 'Mistral Medium is an AI model by Mistral,
                              ideal for tasks requiring moderate reasoning like data extraction,
                               summarizing documents, or writing emails',
        ],
        'mistral-large-latest' => [
            'name' => 'Mistral Large',
            'gateway' => 'mistral',
            'access' => 'pro',
            'max_tokens' => 4096,
            'description' => 'Mistral Large is Mistral AI’s flagship model,
                              excelling in complex tasks requiring large reasoning capabilities or high specialization',
        ],
        'codestral-latest' => [
            'name' => 'Codestral',
            'gateway' => 'mistral',
            'access' => 'pro',
            'max_tokens' => 32768,
            'description' => 'Codestral is a cutting-edge Mistral model
                              that has been specifically designed and optimized for code generation tasks,
                              including fill-in-the-middle and code completion',
        ],

        // OPENAI
        'gpt-3.5-turbo-16k' => [
            'name' => 'GPT-3.5 Turbo 16K',
            'gateway' => 'openai',
            'access' => 'user',
            'max_tokens' => 14000,
            'description' => 'GPT-3.5 Turbo 16K is an OpenAI language model with the same capabilities as the standard GPT-3.5 Turbo
                              but with a larger context window of 16,384 tokens, leading to better predictions',
        ],
        'gpt-4-turbo-preview' => [
            'name' => 'GPT-4 Turbo Preview',
            'gateway' => 'openai',
            'access' => 'user',
            'max_tokens' => 2000,
            'description' => 'GPT-4 Turbo Preview is OpenAI’s latest model, offering advanced reasoning,
                              a 128k context window, and cost-effective token usage',
        ],
        'gpt-4-turbo-2024-04-09' => [
            'name' => 'GPT-4 Turbo 2024-04-09',
            'gateway' => 'openai',
            'access' => 'user',
            'max_tokens' => 2000,
            'description' => 'GPT-4 Turbo 2024-04-09 is OpenAI’s latest model with vision capabilities.
                             It has a context window of 128,000 tokens and its training data is up to date until December 2023',
        ],
        'gpt-4o' => [
            'name' => 'GPT-4o',
            'gateway' => 'openai',
            'access' => 'user',
            'max_tokens' => 2000,
            'description' => "OpenAI's 'most advanced, multimodal flagship model that’s cheaper and faster than GPT-4 Turbo.'",
        ],
        'gpt-4' => [
            'name' => 'GPT-4',
            'gateway' => 'openai',
            'access' => 'pro',
            'max_tokens' => 4096,
            'description' => 'GPT-4 is OpenAI’s advanced AI model, excelling in complex tasks, creative writing, image analysis,
                                 and long-form content creation with improved safety and factual accuracy.',
        ],

        // ANTHROPIC
        'claude-3-haiku-20240307' => [
            'name' => 'Claude Haiku',
            'gateway' => 'anthropic',
            'access' => 'guest',
            'max_tokens' => 4096,
            'description' => 'Haiku is Anthropic’s compact model, designed for instant responsiveness and seamless AI experiences, released on March 7, 2024.',
        ],
        'claude-3-sonnet-20240229' => [
            'name' => 'Claude Sonnet',
            'gateway' => 'anthropic',
            'access' => 'user',
            'max_tokens' => 4096,
            'description' => 'Claude 3 Sonnet is Anthropic’s balanced AI model,
                                excelling in reasoning, multilingual tasks, and visual interpretation, released on February 29, 2024.',
        ],
        'claude-3-opus-20240229' => [
            'name' => 'Claude Opus',
            'gateway' => 'anthropic',
            'access' => 'pro',
            'max_tokens' => 4096,
            'description' => 'Claude 3 Opus, released on February 29, 2024, is Anthropic’s most powerful model,
                             excelling in highly complex tasks and demonstrating fluency and human-like understanding',
        ],

        // PERPLEXITY
        'sonar-small-online' => [
            'name' => 'Sonar Small Online',
            'gateway' => 'perplexity',
            'access' => 'user',
            'max_tokens' => 2000,
            'description' => 'Sonar Small Online is a 7B parameter model by Perplexity,
                              designed for chat completion tasks with a context length of 12,000 tokens',
        ],
        'sonar-medium-online' => [
            'name' => 'Sonar Medium Online',
            'gateway' => 'perplexity',
            'access' => 'pro',
            'max_tokens' => 4096,
            'description' => 'Sonar Medium Online is a search-enhanced model by Perplexity,
                                 surpassing earlier models in cost-efficiency, speed, and performance',
        ],

        // COHERE
        'command-r' => [
            'name' => 'Command-R',
            'gateway' => 'cohere',
            'access' => 'user',
            'max_tokens' => 2000,
            'description' => 'Command R is a large-scale conversational AI model,
                               designed for extensive tasks. It offers a balance between high performance and accuracy,
                               facilitating the transition from prototype to production.',
        ],
        'command-r-plus' => [
            'name' => 'Command-R+',
            'gateway' => 'cohere',
            'access' => 'user',
            'max_tokens' => 4000,
            'description' => 'Command R+ is Cohere’s newest large language model, optimized for conversational interaction
                                 and long-context tasks',
        ],

        // SPIRIT OF SATOSHI
        'satoshi-7b' => [
            'name' => 'Satoshi 7B',
            'gateway' => 'satoshi',
            'access' => 'guest',
            'max_tokens' => 2000,
            'description' => 'Satoshi 7B, developed by LaierTwoLabsInc,
                              is a 7-billion parameter language model fine-tuned on Bitcoin principles, technology, culture,
                                 Austrian economics, and non-woke political perspectives.',
        ],

        // GREPTILE
        'greptile' => [
            'name' => 'Greptile: OA Codebase',
            'gateway' => 'greptile',
            'access' => 'user',
            'max_tokens' => 2000,
            'description' => 'Greptile is an AI platform that enables developers to search and understand complex codebases
                                         in natural language,
                                         enhancing productivity',
        ],

        // VISION
        'gpt-4-vision-preview' => [
            'name' => 'GPT-4 Vision Preview',
            'gateway' => 'openai',
            'access' => 'hidden',
            'max_tokens' => 4000,
            'description' => 'GPT-4 Turbo with Vision is an OpenAI model that analyzes images and provides textual responses,
                                enhancing both visual understanding and natural language processing.',
        ],

    ];

    public static function isProModelSelected($model): bool
    {
        $modelDetails = self::MODELS[$model] ?? null;

        if ($modelDetails) {
            return $modelDetails['access'] === 'pro';
        }

        return false;
    }

    public static function getModelIndicator($model, $userAccess): string
    {
        $modelDetails = self::MODELS[$model] ?? null;

        if ($modelDetails) {
            $requiredAccess = $modelDetails['access'];
            $accessLevels = ['guest', 'user', 'pro'];
            $userAccessIndex = array_search($userAccess, $accessLevels);
            $requiredAccessIndex = array_search($requiredAccess, $accessLevels);

            if ($userAccessIndex < $requiredAccessIndex) {
                if ($requiredAccess === 'pro') {
                    return 'Pro';
                }

                return 'Join';
            }
        }

        return '';
    }

    public static function getModelName($model): string
    {
        return self::MODELS[$model]['name'] ?? 'Unknown Model';
    }

    public static function getModelForThread($thread): string
    {
        if ($thread) {
            $lastMessage = $thread->messages->last();
            if ($lastMessage && ! empty($lastMessage->model)) {
                return $lastMessage->model;
            }
        }

        if (session()->has('selectedModel')) {
            return session('selectedModel');
        }

        return self::getDefaultModel();
    }

    public static function getDefaultModel(): string
    {
        // If user is not logged in, use Mistral Small.
        if (! auth()->check()) {
            return 'mistral-small-latest';
        }

        // If user is logged in and has default_model attribute, use that.
        if (auth()->user()->default_model) {
            return auth()->user()->default_model;
        }

        // If user is logged in and is Pro, use Claude Sonnet.
        if (auth()->check() && auth()->user()->isPro()) {
            return 'claude-3-sonnet-20240229';
        }

        // For authed non-Pro users, use Mistral Medium.
        return 'mistral-medium-latest';
    }

    public static function hasModelAccess($model, $userAccess): bool
    {
        $modelDetails = self::MODELS[$model] ?? null;

        if ($modelDetails) {
            $requiredAccess = $modelDetails['access'];
            $accessLevels = ['guest', 'user', 'pro'];
            $userAccessIndex = array_search($userAccess, $accessLevels);
            $requiredAccessIndex = array_search($requiredAccess, $accessLevels);

            return $userAccessIndex >= $requiredAccessIndex;
        }

        return false;
    }

    public static function getUserAccess(): string
    {
        if (Auth::check()) {
            if (Auth::user()->isPro()) {
                return 'pro';
            }

            return 'user';
        }

        return 'guest';
    }

    public static function getModelPicture($model): ?string
    {
        $modelDetails = self::MODELS[$model] ?? null;

        if ($modelDetails) {
            $gateway = $modelDetails['gateway'];
            $imagePath = 'images/icons/'.$gateway.'.png';

            return url($imagePath);
        }

        return null;
    }

    public static function getModelsForUserTypes(array $types): array
    {
        return array_keys(array_filter(self::MODELS, function ($model) use ($types) {
            return in_array($model['access'], $types);
        }));
    }
}
