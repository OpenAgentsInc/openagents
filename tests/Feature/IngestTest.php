<?php

use App\Models\Embedding;
use App\Services\QueenbeeGateway;
use Illuminate\Http\UploadedFile;
use Spatie\PdfToText\Pdf;


test('can ingest pdf to database as chunked embeddings', function () {

    // Given a PDF, extract the text
    $fileText = Pdf::getText(
        storage_path('app/uploads/0sYtEObUcMCnLo8zAwv7i0cJPfoWTPO4tW7ZblS0.pdf'),
        config('services.pdftotext.path')
    );

    // Clean up and convert into chunks
    $chunks = Str::of($fileText)
        ->split("/\f/") // Splitting the text into pages based on the form feed character
        ->transform(function ($page) {
            return (string) Str::of($page)
                ->replace("\n", " ") // Replacing newline characters with spaces
                ->trim(); // Trimming leading and trailing whitespace
        })
        ->toArray();

    // Create embeddings for each chunk
    $gateway = new QueenbeeGateway();
    $result = $gateway->createEmbedding($chunks);

    // Store the embeddings in a database
    foreach ($chunks as $key=>$chunk) {
        Embedding::query()->create([
            'embedding' => $result[$key]["embedding"],
            'metadata' => [
                'text' => $chunk,
            ]
        ]);
    }

    // Assert that we have the correct number of embeddings (the amount of chunks)
    expect(Embedding::query()->count())->toBe(count($chunks));

    // dd all embeddings
    // dd(Embedding::query()->get()->toArray());
});



// Given a query
// Convert query into embedding
// Find nearest neighbors
// Return results
