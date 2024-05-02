<?php

declare(strict_types=1);

namespace App\AI;

use App\Models\NostrJob;
use App\Models\Thread;
use GuzzleHttp\Client;

class NostrInference
{
    public static function inference(string $model, NostrJob $job, callable $streamFunction, ?Client $httpClient = null): array
    {
        $modelDetails = Models::MODELS[$model] ?? null;

        if ($modelDetails) {

            $gateway = $modelDetails['gateway'];
            $maxTokens = $modelDetails['max_tokens'];
            $thread = Thread::find($job->thread_id);
            $agent = Agent::find($job->agent_id);
            $context = $job->content;

            $preprompt = 'You can use the following CONTEXT to help you answer the user\'s questions.';
            $context = $context;

            $prompt = $agent->prompt."\n".$preprompt."\n".$context;

            $messages = [
                [
                    'role' => 'system',
                    'content' => $prompt,
                    // 'content' => 'You are a helpful assistant on OpenAgents.com. Answer the inquiry from the user.',
                ],
                ...get_truncated_messages($thread, $maxTokens),
            ];

            // Calculate the approximate number of tokens in the messages
            $messageTokens = array_sum(array_map(function ($message) {
                return ceil(str_word_count($message['content']) / 3);
            }, $messages));

            // Adjust the max_tokens value for the completion
            $completionTokens = $maxTokens - $messageTokens;

            if (! $httpClient) {
                $httpClient = new Client();
            }
            $params = [
                'model' => $model,
                'messages' => $messages,
                'max_tokens' => $completionTokens,
                'stream_function' => $streamFunction,
            ];
            switch ($gateway) {
                case 'meta':
                    $client = new TogetherAIGateway($httpClient);
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
                    $params = $thread;
                    break;
                default:
                    dd("Unknown gateway: $gateway");
            }
            $inference = $client->inference($params);
        } else {
            dd("Unknown model: $model");
        }

        return $inference;
    }
}

function get_truncated_messages(Thread $thread, int $maxTokens)
{
    $messages = [];
    $tokenCount = 0;
    $userContent = '';

    foreach ($thread->messages()->orderBy('created_at', 'asc')->get() as $message) {
        if ($message->model !== null) {
            $role = 'assistant';
        } else {
            $role = 'user';
        }

        if ($role === 'user') {
            if (strtolower(substr($message->body, 0, 11)) === 'data:image/') {
                $userContent .= ' <image>';
            } else {
                $userContent .= ' '.$message->body;
            }
        } else {
            if (! empty($userContent)) {
                $messageTokens = ceil(str_word_count($userContent) / 3);

                if ($tokenCount + $messageTokens > $maxTokens) {
                    break; // Stop adding messages if the remaining context is not enough
                }

                $messages[] = [
                    'role' => 'user',
                    'content' => trim($userContent),
                ];

                $tokenCount += $messageTokens;
                $userContent = '';
            }

            if (strtolower(substr($message->body, 0, 11)) === 'data:image/') {
                $content = '<image>';
            } else {
                $content = $message->body;
            }

            $messageTokens = ceil(str_word_count($content) / 3);

            if ($tokenCount + $messageTokens > $maxTokens) {
                break; // Stop adding messages if the remaining context is not enough
            }

            $messages[] = [
                'role' => 'assistant',
                'content' => $content,
            ];

            $tokenCount += $messageTokens;
        }
    }

    if (! empty($userContent)) {
        $messageTokens = ceil(str_word_count($userContent) / 3);

        if ($tokenCount + $messageTokens <= $maxTokens) {
            $messages[] = [
                'role' => 'user',
                'content' => trim($userContent),
            ];
        }
    }

    return $messages;
}
