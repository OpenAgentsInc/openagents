<?php

namespace App\AI;

use OpenAI;

class OpenAIGateway
{
    public function models()
    {
        $client = OpenAI::client(env('OPENAI_API_KEY'));
        $response = $client->models()->list();
        $ids = [];
        foreach ($response->data as $result) {
            $ids[] = $result->id;
        }
        dd($ids);
    }
}
