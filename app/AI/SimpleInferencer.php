<?php

declare(strict_types=1);

namespace App\AI;

use App\Models\Thread;
use GuzzleHttp\Client;
use Yethee\Tiktoken\Encoder;
use Yethee\Tiktoken\EncoderProvider;

class SimpleInferencer
{
    private int $remainingTokens = 0;

    private int $promptTokens = 0;

    private Encoder $encoder;

    public function inference(string $prompt, string $model, Thread $thread, callable $streamFunction, ?Client $httpClient = null, string $systemPrompt = ''): array
    {
        $modelDetails = Models::MODELS[$model] ?? null;

        if ($modelDetails) {
            $gateway = $modelDetails['gateway'];

            $messages = [
                [
                    'role' => 'system',
                    'content' => $systemPrompt ?: 'You are a helpful assistant.',
                ],
                ...$this->getTruncatedMessages($thread, $modelDetails['max_tokens'], $systemPrompt),
            ];

            if (! $httpClient) {
                $httpClient = new Client();
            }
            $params = [
                'model' => $model,
                'messages' => $messages,
                'max_tokens' => $this->remainingTokens,
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
            $inference['prompt_tokens'] = $this->promptTokens;
        }

        return $inference;
    }

    public function getTruncatedMessages(Thread $thread, int $maxTokens, string $systemPrompt = ''): array
    {
        $provider = new EncoderProvider();
        $this->encoder = $provider->getForModel('gpt-4');

        $this->remainingTokens = $maxTokens;

        if ($systemPrompt) {
            $this->remainingTokens -= count($this->encoder->encode($systemPrompt));
        }

        $messages = [];

        foreach (
            $thread->messages()
                ->orderBy('created_at', 'desc')
                ->orderBy('id', 'desc')->get() as $message
        ) {
            $role = is_null($message->model) ? 'user' : 'assistant';
            $this->addMessage($role, $message, $messages);
            if ($this->remainingTokens <= 0) {
                break;
            }
        }

        return $messages;
    }

    private function addMessage(string $role, mixed $message, array &$messages): void
    {
        if (strtolower(substr($message->body, 0, 11)) === 'data:image/') {
            $content = '<image>';
        } else {
            $content = trim($message->body) ?: '<blank>';
        }
        $messageTokens = $role === 'user' ? $message->input_tokens : $message->output_tokens;
        if (! $messageTokens) {
            $messageTokens = count($this->encoder->encode($content));
        }

        // if this is the first message then it is the prompt
        if (count($messages) === 0) {
            $this->promptTokens = $messageTokens;
        }

        if ($messageTokens <= $this->remainingTokens || ($role === 'user')) {
            $this->remainingTokens -= $messageTokens;
            if ($this->remainingTokens < 0) {
                $this->remainingTokens = 0;
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
