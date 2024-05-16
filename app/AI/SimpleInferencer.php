<?php

declare(strict_types=1);

namespace App\AI;

use App\Models\Thread;
use GuzzleHttp\Client;
use Yethee\Tiktoken\Encoder;
use Yethee\Tiktoken\EncoderProvider;

class SimpleInferencer
{
    private static int $remainingTokens = 0;

    private static string $currentPrompt = '';

    private static int $promptTokens = 0;

    private static Encoder $encoder;

    public static function inference(string $prompt, string $model, Thread $thread, callable $streamFunction, ?Client $httpClient = null): array
    {
        $modelDetails = Models::MODELS[$model] ?? null;

        if ($modelDetails) {
            $gateway = $modelDetails['gateway'];
            self::$remainingTokens = $modelDetails['max_tokens'];
            self::$currentPrompt = $prompt;

            $messages = [
                [
                    'role' => 'system',
                    'content' => 'You are a helpful assistant.',
                ],
                ...self::getTruncatedMessages($thread),
            ];

            if (! $httpClient) {
                $httpClient = new Client();
            }
            $params = [
                'model' => $model,
                'messages' => $messages,
                'max_tokens' => self::$remainingTokens,
                'stream_function' => $streamFunction,
            ];
            switch ($gateway) {
                case 'meta':
                    $client = new TogetherAIGateway($httpClient);
                    break;
                case 'groq':
                    $client = new GroqAIGateway($httpClient);
                    break;
                case 'anthropic':
                    $client = new AnthropicAIGateway($httpClient);
                    break;
                case 'mistral':
                    $client = new MistralAIGateway($httpClient);
                    break;
                case 'openai':
                    $client = new OpenAIGateway();
                    break;
                case 'perplexity':
                    $client = new PerplexityAIGateway($httpClient);
                    break;
                case 'cohere':
                    $client = new CohereAIGateway($httpClient);
                    $params['message'] = $prompt;
                    break;
                case 'satoshi':
                    $client = new HuggingfaceAIGateway();
                    break;
                case 'greptile':
                    $client = new GreptileGateway();
                    $params = ['thread' => $thread];
                    break;
                default:
                    dd("Unknown gateway: $gateway");
            }
            $inference = $client->inference($params);
        } else {
            dd("Unknown model: $model");
        }

        // remember prompt tokens so we do not recalculate
        if (count($messages) === 2 && $inference['input_tokens']) {
            $inference['prompt_tokens'] = $inference['input_tokens'];
        } else {
            $inference['prompt_tokens'] = self::$promptTokens;
        }

        return $inference;
    }

    public static function getTruncatedMessages(Thread $thread, ?int $maxTokens = null): array
    {
        if ($maxTokens) {
            self::$remainingTokens = $maxTokens;
        }

        $provider = new EncoderProvider();
        self::$encoder = $provider->getForModel('gpt-4');

        $messages = [];

        foreach (
            $thread->messages()
                ->orderBy('created_at', 'desc')
                ->orderBy('id', 'desc')->get() as $message
        ) {
            $role = is_null($message->model) ? 'user' : 'assistant';
            self::addMessage($role, $message, $messages);
            if (self::$remainingTokens <= 0) {
                break;
            }
        }

        return $messages;
    }

    private static function addMessage(string $role, mixed $message, array &$messages): void
    {
        // if this is the first message then it is the prompt,
        // and it may have been modified by RAG agent
        if (self::$currentPrompt && count($messages) === 0) {
            $message->body = self::$currentPrompt;
            $message->input_tokens = 0;
        }
        if (strtolower(substr($message->body, 0, 11)) === 'data:image/') {
            $content = '<image>';
        } else {
            $content = trim($message->body) ?: '<blank>';
        }
        $messageTokens = $role === 'user' ? $message->input_tokens : $message->output_tokens;
        if (! $messageTokens) {
            $messageTokens = count(self::$encoder->encode($content));
        }

        if (count($messages) === 0) {
            self::$promptTokens = $messageTokens;
        }

        if ($messageTokens <= self::$remainingTokens || ($role === 'user')) {
            self::$remainingTokens -= $messageTokens;
            if (self::$remainingTokens < 0) {
                self::$remainingTokens = 0;
            }

            if (count($messages) && $messages[0]['role'] === $role) {
                if (! str_contains($messages[0]['content'], $content)) {
                    $messages[0]['content'] = $messages[0]['content']."\n".$content;
                }
            } else {
                array_unshift($messages, [
                    'role' => $role,
                    'content' => $content,
                ]);
            }
        }
    }
}
