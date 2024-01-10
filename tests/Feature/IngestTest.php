<?php

use App\Events\EmbeddingCreated;
use App\Jobs\CreateDatapointEmbedding;
use App\Jobs\IngestPDF;
use App\Models\Agent;
use App\Models\Brain;
use App\Models\Datapoint;
use Illuminate\Support\Facades\Event;
use Illuminate\Support\Facades\Queue;

test('ingesting PDF fires embedding job once per page', function () {
    Queue::fake();

    $agent = Agent::factory()->create();
    $brain = Brain::factory()->create(['agent_id' => $agent->id]);

    $path = 'uploads/Seq62Tot1gYvabkLhXOF34d9JCHd1FW9xJdNRIvg.pdf'; // A 4-page PDF
    $job = new IngestPDF($path, $agent, $brain);
    $job->handle();

    Queue::assertPushed(CreateDatapointEmbedding::class, 4);
});

test('embedding job creates datapoint', function () {
    $agent = Agent::factory()->create();
    $brain = Brain::factory()->create(['agent_id' => $agent->id]);

    $job = new CreateDatapointEmbedding("Hello this is an embedding", $agent, $brain);
    $job->handle();

    $this->assertCount(1, Datapoint::all());
});

test('embedding job notifies agent', function () {
    Event::fake();
    Queue::fake();

    $agent = Agent::factory()->create();
    $brain = Brain::factory()->create(['agent_id' => $agent->id]);

    $job = new CreateDatapointEmbedding("Hello this is an embedding", $agent, $brain);
    $job->handle();

    Event::assertDispatched(function (EmbeddingCreated $embeddedJob) use ($agent) {
        return $embeddedJob->agent_id === $agent->id;
    });
});
