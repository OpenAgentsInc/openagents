<?php

use App\Events\EmbeddingCreated;
use App\Jobs\CreateDatapointEmbedding;
use App\Jobs\IngestPDF;
use App\Models\Agent;
use App\Models\Brain;
use App\Models\Datapoint;
use App\Models\File;
use Illuminate\Support\Facades\Event;
use Illuminate\Support\Facades\Queue;

test('ingesting PDF fires embedding job once per page', function () {
    Queue::fake();

    $path = 'uploads/Seq62Tot1gYvabkLhXOF34d9JCHd1FW9xJdNRIvg.pdf'; // A 4-page PDF
    $agent = Agent::factory()->create();
    $brain = Brain::factory()->create(['agent_id' => $agent->id]);
    $file = File::factory()->create(['agent_id' => $agent->id, 'path' => $path]);

    $job = new IngestPDF($path, $agent, $brain, $file);
    $job->handle();

    Queue::assertPushed(CreateDatapointEmbedding::class, 4);
});

test('embedding job creates datapoint', function () {
    $agent = Agent::factory()->create();
    $brain = Brain::factory()->create(['agent_id' => $agent->id]);
    $file = File::factory()->create(['agent_id' => $agent->id]);

    $job = new CreateDatapointEmbedding("Hello this is an embedding", $agent, $brain, $file);
    $job->handle();

    $this->assertCount(1, Datapoint::all());
});

test('embedding job notifies agent with correct agent id', function () {
    Event::fake();
    Queue::fake();

    $agent = Agent::factory()->create();
    $brain = Brain::factory()->create(['agent_id' => $agent->id]);
    $file = File::factory()->create(['agent_id' => $agent->id]);

    $job = new CreateDatapointEmbedding("Hello this is an embedding", $agent, $brain, $file);
    $job->handle();

    Event::assertDispatched(function (EmbeddingCreated $embeddedJob) use ($agent) {
        return $embeddedJob->agent_id === $agent->id;
    });
});

test('embedding job notifies agent with correct file id', function () {
    Event::fake();
    Queue::fake();

    $agent = Agent::factory()->create();
    $brain = Brain::factory()->create(['agent_id' => $agent->id]);
    $file = File::factory()->create(['agent_id' => $agent->id]);

    $job = new CreateDatapointEmbedding("Hello this is an embedding", $agent, $brain, $file);
    $job->handle();

    Event::assertDispatched(function (EmbeddingCreated $embeddedJob) use ($file) {
        return $embeddedJob->file_id === $file->id;
    });
});
