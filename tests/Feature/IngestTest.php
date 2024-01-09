<?php

use App\Jobs\CreateDatapointEmbedding;
use App\Jobs\IngestPDF;
use App\Models\Datapoint;
use Illuminate\Support\Facades\Queue;

// use App\Models\Embedding;
// use App\Services\Parser;

test('ingesting PDF fires embedding job once per page', function () {
    Queue::fake();

    $path = 'uploads/Seq62Tot1gYvabkLhXOF34d9JCHd1FW9xJdNRIvg.pdf'; // A 4-page PDF
    $job = new IngestPDF($path, 1);
    $job->handle();

    Queue::assertPushed(CreateDatapointEmbedding::class, 4);
});



// test('can ingest pdf to database as chunked embeddings', function () {
//     $path = 'uploads/Seq62Tot1gYvabkLhXOF34d9JCHd1FW9xJdNRIvg.pdf';
//     $job = new IngestPDF($path);
//     $job->handle();

//     expect(Datapoint::count())->toBe(4);
// });

// test('can ingest pdf to database as chunked embeddings (old)', function () {
//     $path = 'uploads/iIijnu8yCSUtstPv4D9jM6oLc3rSrqYxrnS3ZCCQ.pdf';
//     $parser = new Parser();
//     $parsed = $parser->parsePdf($path);

//     expect(Embedding::query()->count())->toBe(count($parsed["chunks"]));
// })->group('queenbee');
