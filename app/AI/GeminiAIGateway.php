<?php

namespace App\AI;

use Illuminate\Support\Facades\Http;

class GeminiAIGateway
{
    protected $apiKey;

    protected $baseUrl = 'https://generativelanguage.googleapis.com';

    protected $defaultModel = 'gemini-pro'; // Default text-only model

    protected $newModel = 'gemini-1.5-pro-latest';

    protected $visionModel = 'gemini-pro-vision'; // Model for text and image prompts

    public function __construct()
    {
        $this->apiKey = env('GEMINI_API_KEY');
    }

    public function inference(string|array $prompt, ?string $model = null): array
    {
        // Determine the model to use based on prompt type and optional parameter
        if (is_array($prompt) && array_key_exists('contents', $prompt)) {
            // Assume prompts with 'contents' key contain image data, use vision model
            $modelPath = $this->visionModel;
        } else {
            // Use default text-only model or specified model
            $modelPath = $model === 'new' ? $this->newModel : $this->defaultModel;
        }

        $url = "{$this->baseUrl}/v1beta/models/{$modelPath}:generateContent?key={$this->apiKey}";

        $blob = [
            'contents' => [
                [
                    'parts' => [
                        ['text' => $prompt],
                    ],
                ],
            ],
        ];

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
        ])->timeout(120)->post($url, $blob);

        dump($response->json());

        return $response->successful() ? $response->json() : [
            'error' => 'Failed to generate inference',
            'details' => $response->json(),
        ];
    }

    public function chat(array $messages, ?string $model = null): array
    {
        $modelPath = $model === 'new' ? $this->newModel : $this->defaultModel;

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
        ])->timeout(120)->post("{$this->baseUrl}/v1beta/models/{$modelPath}:generateContent?key={$this->apiKey}", [
            'contents' => array_map(function ($message) {
                return [
                    'role' => $message['role'],
                    'parts' => [
                        ['text' => $message['text']],
                    ],
                ];
            }, $messages),
        ]);

        return $response->successful() ? $response->json() : [];
    }

    public function uploadFile(string $filePath, ?string $displayName = null): array
    {
        $apiKey = $this->apiKey;
        $baseUrl = $this->baseUrl;

        // Determine MIME type
        $mimeType = mime_content_type($filePath);

        // Read file content
        $fileContent = file_get_contents($filePath);
        $fileSize = filesize($filePath);

        // Prepare initial request data
        $metadata = [
            'file' => [
                'displayName' => $displayName,
            ],
        ];

        // Start resumable upload
        $startUrl = "{$baseUrl}/upload/v1beta/files?key={$apiKey}";
        $startResponse = Http::withHeaders([
            'X-Goog-Upload-Protocol' => 'resumable',
            'X-Goog-Upload-Command' => 'start',
            'X-Goog-Upload-Header-Content-Length' => $fileSize,
            'X-Goog-Upload-Header-Content-Type' => $mimeType,
            'Content-Type' => 'application/json',
        ])->post($startUrl, $metadata);

        if (! $startResponse->successful()) {
            return [
                'error' => 'Failed to initiate file upload',
                'details' => $startResponse->json(),
            ];
        }

        // dd with the response headers
        //        dd($startResponse->headers());

        // Extract upload URL from response header
        $uploadUrl = $startResponse->header('x-goog-upload-url');

        // Upload file content in chunks
        $chunkSize = 8388608; // 8 MiB
        $numChunks = ceil($fileSize / $chunkSize);

        for ($i = 1; $i <= $numChunks; $i++) {
            $offset = ($i - 1) * $chunkSize;
            $chunkData = substr($fileContent, $offset, $chunkSize);

            $uploadCommand = 'upload';
            if ($i === $numChunks) {
                $uploadCommand .= ', finalize';
            }

            //            dd([
            //                'Content-Length' => strlen($chunkData),
            //                'X-Goog-Upload-Offset' => $offset,
            //                'X-Goog-Upload-Command' => $uploadCommand,
            //            ]);
            // [
            //                "Content-Length" => 79067
            //  "X-Goog-Upload-Offset" => 0
            //  "X-Goog-Upload-Command" => "upload"
            //]

            $chunkResponse = Http::withHeaders([
                'Content-Length' => strlen($chunkData),
                'X-Goog-Upload-Offset' => $offset,
                'X-Goog-Upload-Command' => $uploadCommand,
            ])->withBody($chunkData, 'application/octet-stream')->post($uploadUrl);

            if (! $chunkResponse->successful()) {
                dd($chunkResponse);

                return [
                    'error' => 'Failed to upload file chunk',
                    'details' => $chunkResponse->json(),
                ];
            }
        }

        // Retrieve uploaded file metadata
        $fileResponse = Http::get("{$baseUrl}/v1beta/files/{$startResponse->json()['file']['name']}?key={$apiKey}");

        return $fileResponse->successful() ? $fileResponse->json() : [
            'error' => 'Failed to retrieve uploaded file metadata',
            'details' => $fileResponse->json(),
        ];
    }

    public function listFiles(?string $pageToken = null): array
    {
        $apiKey = $this->apiKey;
        $baseUrl = $this->baseUrl;

        $url = "{$baseUrl}/v1beta/files?key={$apiKey}";

        if ($pageToken) {
            $url .= "&pageToken={$pageToken}";
        }

        $response = Http::get($url);

        return $response->successful() ? $response->json() : [
            'error' => 'Failed to list files',
            'details' => $response->json(),
        ];
    }
}
