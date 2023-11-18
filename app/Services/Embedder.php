<?php

namespace App\Services;

use App\Models\File;

class Embedder
{
    public function createEmbeddingsForFolder($folder)
    {
        $files = scandir($folder);
        $files = array_diff($files, array('.', '..'));
        $files = array_values($files);
        $embeddings = [];
        foreach ($files as $file) {
            print_r("Creating embeddings for " . $folder . '/' . $file . "\n");
            $fileModel = File::factory()->create([
              'path' => $folder . '/' . $file,
            ]);
            $fileModel->createEmbeddings();
        }
    }
}
