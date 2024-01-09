<?php

use App\Jobs\IngestPDF;
use App\Models\Datapoint;

// use App\Models\Embedding;
// use App\Services\Parser;

test('can ingest pdf to database as chunked embeddings', function () {
    $path = 'uploads/0sYtEObUcMCnLo8zAwv7i0cJPfoWTPO4tW7ZblS0.pdf';
    $job = new IngestPDF($path);
    $job->handle();

    expect(Datapoint::count())->toBe(4);
});

// test('can ingest pdf to database as chunked embeddings (old)', function () {
//     $path = 'uploads/iIijnu8yCSUtstPv4D9jM6oLc3rSrqYxrnS3ZCCQ.pdf';
//     $parser = new Parser();
//     $parsed = $parser->parsePdf($path);

//     expect(Embedding::query()->count())->toBe(count($parsed["chunks"]));
// })->group('queenbee');
