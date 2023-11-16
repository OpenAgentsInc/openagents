<?php

namespace App\Http\Controllers;

use App\Services\Searcher;

class QueryController extends Controller
{
    public function store()
    {
        // validate that request has file_id and query
        request()->validate([
          'file_id' => 'required',
          'query' => 'required',
        ]);

        $file_id = request('file_id');
        $query = request('query');

        $searcher = new Searcher();
        $res = $searcher->query($file_id, $query);

        if ($res['ok'] && isset($res['results'])) {
            return response()->json([
                'ok' => true,
                'results' => $res['results'],
            ], 200);
        }

        // Return an error response if the query was not successful
        return response()->json([
            'ok' => false,
            'error' => 'Query failed or returned no data',
        ], 400);
    }
}
