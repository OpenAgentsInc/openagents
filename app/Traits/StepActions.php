<?php

namespace App\Traits;

use App\Models\Embedding;
use App\Services\QueenbeeGateway;
use Pgvector\Laravel\Vector;

trait StepActions
{
    public function validation($input)
    {
        // Expect an array with key input and value string, nothing else.
        // echo "Validating input: \n";
        // \print_r($input);

        // Check if input is an array
        if (!is_array($input)) {
            echo "Input is not an array.\n";
            dd($input);
        }

        // Check if input has only one key
        if (count($input) !== 1) {
            echo "Input has more than one key.\n";
            dd($input);
        }

        // Check if input has key input
        if (!array_key_exists('input', $input)) {
            echo "Input does not have key input.\n";
            dd($input);
        }

        // Check if input[input] is a string
        if (!is_string($input['input'])) {
            echo "Input is not a string.\n";
            dd($input);
        }

        return $input;
    }

    public function embedding($input)
    {
        $input = $input['input'];
        // Check if input is a string
        if (!is_string($input)) {
            echo "Embedding input is not a string.\n";
            dd($input);
        }

        $gateway = new QueenbeeGateway();
        $result = $gateway->createEmbedding($input);
        $embedding = $result[0]['embedding'];

        return [
            'embedding' => $embedding
        ];
    }

    public function similarity_search($input, $take = 8)
    {
        $embedding = $input['embedding'];
        if (!is_array($embedding)) {
            echo "Similarity search input is not an array.\n";
            dd($embedding);
        }

        $vector = new Vector($embedding);

        $searchResults = Embedding::query() // should be what?
            ->orderByRaw('embedding <-> ?', [$vector])
            ->take($take)
            ->pluck('metadata');

        return $searchResults;
    }

    public function inference($input)
    {
        return $input;
    }
}
