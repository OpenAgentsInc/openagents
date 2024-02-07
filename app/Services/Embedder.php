<?php

namespace App\Services;

class Embedder
{
    public static function createFakeEmbedding()
    {
        return array_fill(0, 768, 0);
    }
}
