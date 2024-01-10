<?php

use App\Jobs\CreateDatapointEmbedding;
use App\Jobs\IngestPDF;
use App\Models\Brain;
use App\Models\Datapoint;
use Illuminate\Support\Facades\Queue;

test('ingesting PDF fires embedding job once per page', function () {
    Queue::fake();

    $brain = Brain::factory()->create();

    $path = 'uploads/Seq62Tot1gYvabkLhXOF34d9JCHd1FW9xJdNRIvg.pdf'; // A 4-page PDF
    $job = new IngestPDF($path, $brain);
    $job->handle();

    Queue::assertPushed(CreateDatapointEmbedding::class, 4);
});

test('embedding job creates datapoint', function () {
    $brain = Brain::factory()->create();

    $job = new CreateDatapointEmbedding("Hello this is an embedding", $brain);
    $job->handle();

    $this->assertCount(1, Datapoint::all());
})->group('integration');
