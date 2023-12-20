<?php

namespace App\Traits;

use App\Services\OpenAIGateway;

trait UsesInference
{
    protected $inference;

    public function initializeInference()
    {
        $this->inference = $this;
    }

    public function summarizeUnstructuredData($systemPrompt, $data)
    {
        $gateway = new OpenAIGateway();

        // Determine the length of json_encode($data) and truncate if necessary
        $dataLength = strlen(json_encode($data));
        $maxDataLength = 20000;
        if ($dataLength > $maxDataLength) {
            $data = substr(json_encode($data), 0, $maxDataLength);
        } else {
            $data = json_encode($data);
        }

        $response = $gateway->makeChatCompletion([
            'model' => 'gpt-4',
            'messages' => [
                ['role' => 'system', 'content' => $systemPrompt],
                ['role' => 'user', 'content' => "Summarize this data. Ignore basic repo metadata like stars and such: " . $data],
            ],
        ]);

        $output = $response['choices'][0]['message']['content'];

        return $output;
    }
}
