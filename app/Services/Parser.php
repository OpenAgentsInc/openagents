<?php

namespace App\Services;

use App\Models\Embedding;
use App\Models\File;
use App\Services\QueenbeeGateway;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Str;
use Spatie\PdfToText\Pdf;

class Parser
{
    public function parsePdf($path)
    {
        // Given a PDF, extract the text
        $fileText = Pdf::getText(
            storage_path("app/" . $path),
            config('services.pdftotext.path')
        );

        // First create a new File
        $file = File::query()->create([
          'path' => $path,
        ]);

        // Clean up and convert into chunks
        // Clean up and convert into chunks of max 1024 characters
        // $chunks = [];
        // $length = 1024; // Max length of each chunk
        // while (strlen($fileText) > 0) {
        //     $chunk = Str::of($fileText)->limit($length, '');
        //     $chunks[] = (string) $chunk->replace("\n", " ")->trim();
        //     $fileText = substr($fileText, strlen($chunk)); // Remove processed chunk
        // }
        $chunks = Str::of($fileText)
            ->split("/\f/") // Splitting the text into pages based on the form feed character
            ->transform(function ($page) {
                return (string) Str::of($page)
                    ->replace("\n", " ") // Replacing newline characters with spaces
                    ->trim(); // Trimming leading and trailing whitespace
            })
            ->toArray();

        $smallerChunks = [];
        $length = 1024; // Max length of each chunk
        foreach ($chunks as $chunk) {
            while (strlen($chunk) > 0) {
                $smallerChunk = Str::of($chunk)->limit($length, '');
                $smallerChunks[] = (string) $smallerChunk->replace("\n", " ")->trim();
                $chunk = substr($chunk, strlen($smallerChunk)); // Remove processed chunk
            }
        }

        Log::info($smallerChunks);
        // and log the number of smallerChunks
        Log::info(count($smallerChunks));

        // Send only the first 3 chunks to Queenbee
        $smallerChunks = $chunks; // array_slice($smallerChunks, 0, 8);
        Log::info($smallerChunks);

        // Create embeddings for each chunk
        $gateway = new QueenbeeGateway();
        $result = $gateway->createEmbedding($smallerChunks);
        Log::info($result);

        $chunks = $smallerChunks;

        // Store the embeddings in a database
        foreach ($chunks as $key=>$chunk) {
            Embedding::query()->create([
                'file_id' => $file->id,
                'embedding' => $result[$key]["embedding"],
                'metadata' => [
                    'text' => $chunk,
                ]
            ]);
        }

        return [
            'file_id' => $file->id,
            'chunks' => $chunks,
            'embeddings' => $result,
        ];
    }
}
