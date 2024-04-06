<?php

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use GuzzleHttp\Psr7\Request;
use Psr\Http\Message\ResponseInterface;

class AnthropicGateway
{
    private $client;

    private $streamFunction;

    public function __construct()
    {
        $this->client = new Client([
            // Base URI is used with relative requests
            'base_uri' => 'https://api.anthropic.com/v1',
            // You can set any number of default request options.
            'headers' => [
                'Content-Type' => 'application/json',
                'x-api-key' => getenv('ANTHROPIC_API_KEY'), // Make sure to set your API key in your .env file
                'anthropic-version' => '2023-06-01',
                'anthropic-beta' => 'messages-2023-12-15',
            ],
        ]);
    }

    public function streamMessages($model, $messages, $maxTokens, $streamFunction)
    {
        $this->streamFunction = $streamFunction;

        $data = [
            'model' => $model,
            'messages' => $messages,
            'max_tokens' => $maxTokens,
            'stream' => true,
        ];

        $request = new Request('POST', '/messages', [], json_encode($data));

        try {
            $this->client->sendAsync($request)->then(
                function (ResponseInterface $res) {
                    $stream = $res->getBody();
                    while (! $stream->eof()) {
                        $line = $stream->readLine();
                        $data = json_decode($line, true);
                        if ($data) {
                            call_user_func($this->streamFunction, $data);
                        }
                    }
                },
                function (RequestException $e) {
                    echo $e->getMessage()."\n";
                    echo $e->getRequest()->getMethod();
                }
            )->wait();
        } catch (RequestException $e) {
            // Handle exception or error
            echo 'Error: '.$e->getMessage();
        }
    }
}
