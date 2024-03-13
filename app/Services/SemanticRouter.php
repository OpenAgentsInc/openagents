<?php

namespace App\Services;

class SemanticRouter
{
    public static function route($vectorizedInput)
    {
        dd($vectorizedInput);
    }

    private function cosineSimilarity($vectorA, $vectorB)
    {
        $dotProduct = array_sum(array_map(function ($a, $b) {
            return $a * $b;
        }, $vectorA, $vectorB));
        $normA = sqrt(array_sum(array_map(function ($a) {
            return $a * $a;
        }, $vectorA)));
        $normB = sqrt(array_sum(array_map(function ($b) {
            return $b * $b;
        }, $vectorB)));

        return $dotProduct / ($normA * $normB);
    }

    private function getRoutes()
    {
        return [
            'inference' => 'llmInference',
            'default' => 'defaultRoute',
        ];
    }
}
