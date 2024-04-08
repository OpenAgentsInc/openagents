<?php

namespace App\AI;

use OpenAI;
use Yethee\Tiktoken\EncoderProvider;

class OpenAIGateway
{
    public function __construct()
    {
        $this->client = OpenAI::client(env('OPENAI_API_KEY'));
    }

    public function models()
    {
        $response = $this->client->models()->list();
        $ids = [];
        foreach ($response->data as $result) {
            $ids[] = $result->id;
        }
        dd($ids);
    }

    public function stream($params)
    {
        $model = $params['model'];
        $messages = $params['messages'];
        $max_tokens = $params['max_tokens'];
        $stream_function = $params['stream_function'];

        $message = '';

        $stream = $this->client->chat()->createStreamed([
            'model' => $model,
            'messages' => $messages,
            'max_tokens' => $max_tokens,
        ]);

        foreach ($stream as $response) {
            $stream_function($response);
            $message .= $response['choices'][0]['delta']['content'] ?? '';
        }

        // OpenAI doesn't return token counts during streams (wtf) - https://github.com/openai-php/client/issues/186#issuecomment-2033185221
        // So we have to count the tokens ourselves
        $provider = new EncoderProvider();

        $encoder = $provider->getForModel('gpt-4'); // gpt-3.5.turbo & gpt-4 have the same tokenization
        $outputTokens = $encoder->encode($message);

        // Calculate input tokens by extracting the content from the messages and counting # of tokens (approximation)
        $content = '';
        foreach ($messages as $messagetocount) {
            $content .= $messagetocount['content'].' ';
        }
        $inputTokens = $encoder->encode($content);

        return [
            'content' => $message,
            'input_tokens' => count($inputTokens),
            'output_tokens' => count($outputTokens),
        ];
    }
}
