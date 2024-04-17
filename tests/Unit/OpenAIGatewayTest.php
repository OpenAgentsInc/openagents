<?php
declare(strict_types=1);

use App\AI\OpenAIGateway;

test('OpenAIGateway handles OpenAI responses correctly', function () {
    $prompt = 'What is the capital of France?';
    $inputTokens = 8;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 6;

    $parameters = [
        'model' => 'gpt-3.5-turbo-16k',
        'messages' => [
            ['role' => 'user', 'content' => $prompt]
        ],
        'stream_function' => function ($response) use ($answer) {
            expect($response['choices'][0]['delta']['content'])->toEqual($answer);
        },
        'max_tokens' => 2000
    ];

    $chatClient = new ChatClientMock([[
        'choices' => [[
            'delta' => [
                'content' => $answer
            ]
        ]]
    ]]);
    $gateway = new OpenAIGateway($chatClient);

    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});

class ChatClientMock
{
    protected array $response = [];

    public function __construct(array $mockResponse)
    {
        $this->response = $mockResponse;
    }

    public function chat()
    {
        return new class($this->response) extends ChatClientMock
        {
            public function createStreamed(): array
            {
                return $this->response;
            }
        };
    }
}
