<?php

use App\Models\Embedding;
use App\Services\Parser;
use App\Services\QueenbeeGateway;
use Illuminate\Http\UploadedFile;
use Spatie\PdfToText\Pdf;

test('can ingest pdf to database as chunked embeddings', function () {
  $path = 'uploads/0sYtEObUcMCnLo8zAwv7i0cJPfoWTPO4tW7ZblS0.pdf';
  $parser = new Parser();
  $parsed = $parser->parsePdf($path);

  expect(Embedding::query()->count())->toBe(count($parsed["chunks"]));
});
