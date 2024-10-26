<?php

declare(strict_types=1);

namespace App\Traits;

use Psr\Http\Message\ResponseInterface;
use Illuminate\Support\Facades\Log;

trait StreamingTrait
{
    private array $data = [];

    protected function extractData(ResponseInterface $response, string $responseBody, bool $stream, ?callable $streamFunction): array
    {
        if ($stream) {
            return $this->extractFromStream($response, $streamFunction);
        }

        Log::info('Raw response body', ['body' => $responseBody]);

        if (empty($responseBody)) {
            Log::error('Empty response body received');
            throw new \RuntimeException('Empty response body received from the API');
        }

        $responseData = json_decode($responseBody, true);

        if ($responseData === null) {
            $jsonError = json_last_error();
            $jsonErrorMsg = json_last_error_msg();
            Log::error('Failed to decode JSON response', [
                'response' => $responseBody,
                'json_error_code' => $jsonError,
                'json_error_message' => $jsonErrorMsg,
            ]);
            throw new \RuntimeException("Failed to decode JSON response. Error: {$jsonErrorMsg} (Code: {$jsonError})");
        }

        return $this->extractFromJson($responseData);
    }

    protected function extractFromJson(array $responseData): array
    {
        $content = '';
        if (isset($responseData['choices'][0]['message']['content'])) {
            $content = $responseData['choices'][0]['message']['content'];
        } elseif (isset($responseData['choices'][0]['message']['tool_calls'])) {
            $content = json_encode($responseData['choices'][0]['message']['tool_calls']);
        }

        return [
            'content' => $content,
            'output_tokens' => $responseData['usage']['completion_tokens'] ?? 0,
            'input_tokens' => $responseData['usage']['prompt_tokens'] ?? 0,
        ];
    }

    protected function extractFromStream($response, ?callable $streamFunction): array
    {
        $stream = $response->getBody();

        $this->data = [
            'content' => '',
            'input_tokens' => 0,
            'output_tokens' => 0,
        ];

        foreach ($this->readStream($stream) as $event) {
            Log::info('Streaming event received', ['event' => $event]);
            $this->extractTokens($event, $streamFunction);
        }

        Log::info('Streaming completed', ['final_data' => $this->data]);
        return $this->data;
    }

    protected function extractTokens(array $event, ?callable $streamFunction)
    {
        if (isset($event['choices'][0]['delta']['content'])) {
            $this->data['content'] .= $event['choices'][0]['delta']['content'];
            if ($streamFunction !== null) {
                $streamFunction($event['choices'][0]['delta']['content']);
            }
        } elseif (isset($event['choices'][0]['delta']['tool_calls'])) {
            $toolCallContent = json_encode($event['choices'][0]['delta']['tool_calls']);
            $this->data['content'] .= $toolCallContent;
            if ($streamFunction !== null) {
                $streamFunction($toolCallContent);
            }
        }
        $usage = $event['usage'] ?? $event['x_groq']['usage'] ?? [];
        if (isset($usage['prompt_tokens'])) {
            $this->data['input_tokens'] = $usage['prompt_tokens'];
        }
        if (isset($usage['completion_tokens'])) {
            $this->data['output_tokens'] = $usage['completion_tokens'];
        }
    }

    protected function readStream($stream)
    {
        $buffer = '';
        while (! $stream->eof()) {
            $buffer .= $stream->read(1024);
            while (($pos = strpos($buffer, "\n")) !== false) {
                $line = substr($buffer, 0, $pos);
                $buffer = substr($buffer, $pos + 1);

                if (str_starts_with($line, 'data: ')) {
                    $data = json_decode(trim(substr($line, 5)), true);
                    if ($data) {
                        yield $data;
                    }
                } elseif (str_ends_with($line, '}')) {
                    $data = json_decode(trim($line), true);
                    if ($data) {
                        yield $data;
                    }
                }
            }
        }
    }
}
