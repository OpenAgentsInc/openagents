<?php

declare(strict_types=1);

namespace App\AI;

use Aws\BedrockRuntime\BedrockRuntimeClient;
use GuzzleHttp\Exception\RequestException;
use Illuminate\Support\Facades\Log;
use App\Services\ToolService;
use App\AI\Traits\BedrockMessageFormatting;

class BedrockAIGateway
{
    use BedrockMessageFormatting;

    private BedrockRuntimeClient $bedrockClient;
    private ToolService $toolService;

    public function __construct(ToolService $toolService)
    {
        $this->bedrockClient = new BedrockRuntimeClient([
            'version' => '2023-09-30',
            'region' => env('AWS_DEFAULT_REGION', 'us-east-1'),
            'credentials' => [
                'key' => env('AWS_ACCESS_KEY_ID'),
                'secret' => env('AWS_SECRET_ACCESS_KEY'),
            ],
        ]);
        $this->toolService = $toolService;
    }

    public function inference(array $params): array
    {
        return $this->converse($params);
    }

    public function converse(array $params): array
    {
        $requestBody = $this->prepareRequestBody($params);
        Log::info('Bedrock API request', ['requestBody' => $requestBody]);

        try {
            $result = $this->bedrockClient->converse($requestBody);
            // Log::info('Bedrock API response', ['result' => $result]);
            // return $result;
            $decodedBody = $this->processApiResponse($result);
            return $this->formatResponse($decodedBody);
        } catch (RequestException $e) {
            Log::error("Bedrock API error: " . $e->getMessage());
            throw new \Exception("Bedrock API error: " . $e->getMessage());
        }
    }

    private function prepareRequestBody(array $params): array
    {
        $modelId = $params['model'] ?? 'anthropic.claude-3-sonnet-20240229-v1:0';
        $converter = new BedrockMessageConverter();
        $convertedMessages = $converter->convertToBedrockChatMessages($params['messages']);

        $requestBody = [
            'modelId' => $modelId,
            'contentType' => 'application/json',
            'accept' => 'application/json',
            'anthropic_version' => 'bedrock-2023-05-31',
            'messages' => $convertedMessages['messages'],
            'max_tokens' => $params['max_tokens'] ?? 2200,
            'temperature' => $params['temperature'] ?? 0.7,
            'top_p' => $params['top_p'] ?? 1,
        ];

        if ($convertedMessages['system'] !== null) {
            $requestBody['system'] = $convertedMessages['system'];
        }

        $this->addToolConfiguration($requestBody);

        Log::info("We have prepared the request body", ['requestBody' => $requestBody]);

        return $requestBody;
    }

    private function addToolConfiguration(array &$requestBody): void
    {
        $toolConfig = $this->toolService->getToolDefinitions();
        if (!empty($toolConfig['tools'])) {
            $requestBody['toolConfig'] = $toolConfig;
        }
    }

    private function processApiResponse($result): array
    {
        // Log::info('Bedrock API response', ['result' => $result]);
        $jsonString = $this->extractJsonFromResult($result);
        $decodedBody = json_decode($jsonString, true);
        // Log::info('Bedrock API decoded response', ['decodedBody' => $decodedBody]);
        return $decodedBody;
    }

    private function extractJsonFromResult($result): string
    {
        $resultString = (string) $result;
        $jsonStart = strpos($resultString, '{');
        $jsonEnd = strrpos($resultString, '}');

        if ($jsonStart === false || $jsonEnd === false) {
            throw new \Exception("Unable to extract JSON from Bedrock API response");
        }

        return substr($resultString, $jsonStart, $jsonEnd - $jsonStart + 1);
    }
}
