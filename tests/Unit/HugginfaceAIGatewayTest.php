<?php
declare(strict_types=1);

use App\AI\HuggingfaceAIGateway;
use Illuminate\Support\Facades\Http;

test('CohereAIGateway handles mistral responses correctly', function () {
    $prompt = 'What is the capital of France?';
    $answer = 'Capital of France is Paris.';

    $parameters = [
        'messages' => [
            ['role' => 'user', 'content' => $prompt]
        ]
    ];

    $mockResponse = [[
        'generated_text' => $answer
    ]];

    Http::shouldReceive('withHeaders')
                    ->once()
                    ->andReturn(new HttpPostMock($mockResponse));

    $gateway = new HuggingfaceAIGateway();
    
    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual(0);
    expect($result['output_tokens'])->toEqual(0);
});

class HttpPostMock
{
    protected array $response = [];

    public function __construct(array $mockResponse)
    {
        $this->response = $mockResponse;
    }

    public function post()
    {
        return new class($this->response) extends ChatClientMock
        {
            public function successful(): bool
            {
                return true;
            }

            public function json(): array
            {
                return $this->response;
            }
        };
    }
}

