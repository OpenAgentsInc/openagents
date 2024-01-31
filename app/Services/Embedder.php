<?php

namespace App\Services;

use App\Models\File;

class Embedder
{
    public static function createFakeEmbedding()
    {
        return array_fill(0, 768, 0);
    }
}
