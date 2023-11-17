<?php

namespace App\Services;

use App\Models\Embedding;
use Illuminate\Support\Facades\Log;
use Pgvector\Laravel\Vector;

class Searcher
{
    public function query($file_id, $query)
    {
        $gateway = new QueenbeeGateway();
        $result = $gateway->createEmbedding($query);

        $embedding = new Vector($result[0]["embedding"]);

        // Execute the query and get the results
        $searchResults = Embedding::query()
            ->where('file_id', $file_id)
            ->orderByRaw('embedding <-> ?', [$embedding])
            ->take(1)
            ->pluck('metadata');

        $summary = $this->summarize($searchResults, $query);

        // Format the response with the actual search results
        return [
          "ok" => true,
          "results" => $searchResults,
          "summary" => $summary,
        ];
    }

    // $data is an array of objects (json?) with key text
    public function summarize($data, $query) {
      // Generate context from the $data, which is an array of objects
      $context = '';
      for ($i = 0; $i < count($data); $i++) {
        $context .= $data[$i]['text'] . "\n---\n";
      }

      $gateway = new QueenbeeGateway();
      $response = $gateway->makeChatCompletion([
        'model' => $gateway->defaultModel(),
        'messages' => [
          ['role' => 'system', 'content' => 'You are a helpful assistant. Answer the user\'s question based only on the following context: ' . $context],
          ['role' => 'user', 'content' => $query],
        ],
      ]);

      Log::info($response);

      return $response["choices"][0]["message"]["content"];
    }
}
