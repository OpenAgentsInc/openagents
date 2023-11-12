<?php

namespace App\Http\Controllers;

use App\Services\Vectara;

class QueryController extends Controller
{
    public function store()
    {

        // validate that request has corpus_id and query
        request()->validate([
          'corpus_id' => 'required',
          'query' => 'required',
        ]);

        $corpus_id = request('corpus_id');
        $query = request('query');

        $vectara = new Vectara();
        $res = $vectara->query($corpus_id, $query);

        // Check if the query was successful and has data
        if ($res['ok'] && isset($res['data']['responseSet'][0]['response'])) {
            $queryResults = $res['data']['responseSet'][0]['response'];
            $parsedResults = array_map(function ($item) {
                return [
                    'text' => $item['text'],
                    'score' => $item['score'],
                    'metadata' => array_column($item['metadata'], 'value', 'name')
                ];
            }, $queryResults);

            return response()->json([
                'ok' => true,
                'results' => $parsedResults,
                'summary' => $res['summary'],
            ], 200);
        }

        // Return an error response if the query was not successful
        return response()->json([
            'ok' => false,
            'error' => 'Query failed or returned no data',
        ], 400);
    }
}
