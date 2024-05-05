<?php

namespace App\AI;

use App\Models\Thread;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Session;

class GreptileGateway implements GatewayInterface
{
    private $greptileApiKey;

    private $githubToken;

    private $greptileBaseUrl = 'https://api.greptile.com/v2';

    public function __construct()
    {
        $this->greptileApiKey = config('services.greptile.api_key');
        $this->githubToken = config('services.github.token');
    }

    public function createRepository($repository = 'OpenAgentsInc/openagents')
    {
        $response = Http::withHeaders([
            'Authorization' => 'Bearer '.$this->greptileApiKey,
            'X-Github-Token' => $this->githubToken,
            'Content-Type' => 'application/json',
        ])->post($this->greptileBaseUrl.'/repositories', [
            'remote' => 'github',
            'repository' => $repository,
        ]);

        return $response->json();
    }

    public function search($input)
    {
        $data = [
            'query' => $input,
            'repositories' => [
                [
                    'branch' => 'main',
                    'repository' => 'OpenAgentsInc/openagents',
                ],
            ],
            'sessionId' => Session::getId(),
        ];

        $response = Http::withHeaders([
            'Authorization' => 'Bearer '.$this->greptileApiKey,
            'Content-Type' => 'application/json',
            'X-GitHub-Token' => $this->githubToken,
        ])->timeout(120)->post($this->greptileBaseUrl.'/search', $data);

        if ($response->successful() && $response->body()) {
            $json = $response->json();

            return json_encode($json);
        } else {
            // Handle error or empty response
            dd($response->body());
        }
    }

    public function getRepository($repositoryId = 'github:main:OpenAgentsInc/openagents')
    {
        $encodedRepositoryId = rawurlencode($repositoryId);

        $response = Http::withHeaders([
            'Authorization' => 'Bearer '.$this->greptileApiKey,
            'Accept' => 'application/json',
        ])->get($this->greptileBaseUrl.'/repositories/'.$encodedRepositoryId);

        if ($response->successful() && $response->body()) {
            return $response->json();
        } else {
            // Handle error or empty response
            dd($response->body());
        }
    }

    public function inference(array $params): array
    {
        $messages = self::getFormattedMessages($params['thread']);

        $data = [
            'messages' => $messages,
            'repositories' => [
                [
                    'branch' => 'main',
                    'repository' => 'OpenAgentsInc/openagents',
                ],
            ],
            'sessionId' => Session::getId(),
        ];

        $response = Http::withHeaders([
            'Authorization' => 'Bearer '.$this->greptileApiKey,
            'Content-Type' => 'application/json',
            'X-GitHub-Token' => $this->githubToken,
        ])->timeout(90)->post($this->greptileBaseUrl.'/query', $data);

        if ($response->successful() && $response->body()) {
            $json = $response->json();

            return [
                'content' => $json['message'] ?? '',
                'output_tokens' => 0,
                'input_tokens' => 0,
            ];

        } else {
            // Handle error or empty response
            dd($response->body());
        }
    }

    private static function getFormattedMessages(Thread $thread)
    {
        $messages = [];
        $userContent = '';
        $prevRole = null;

        foreach ($thread->messages()->orderBy('created_at', 'asc')->get() as $message) {
            $role = $message->model !== null ? 'assistant' : 'user';
            $content = strtolower(substr($message->body, 0, 11)) === 'data:image/' ? '<image>' : $message->body;

            if ($role === 'user') {
                $userContent .= ' '.$content;
            } else {
                if (! empty($userContent)) {
                    $messages[] = [
                        'id' => 'user-message-'.count($messages),
                        'content' => trim($userContent),
                        'role' => 'user',
                    ];
                    $userContent = '';
                }

                $messages[] = [
                    'id' => 'assistant-message-'.count($messages),
                    'content' => $content,
                    'role' => 'assistant',
                ];
            }

            $prevRole = $role;
        }

        if (! empty($userContent)) {
            $messages[] = [
                'id' => 'user-message-'.count($messages),
                'content' => trim($userContent),
                'role' => 'user',
            ];
        }

        return $messages;
    }
}
