<?php

use App\Models\Agent;
use App\Models\Brain;
use App\Models\Thought;
use App\Services\Embedder;

test('it has a body', function () {
    $thought = Thought::factory()->create(['body' => 'Hello world!']);
    expect($thought->body)->toBe('Hello world!');
});

test('body cannot be null', function () {
    expect(fn () => Thought::factory()->create(['body' => null]))
        ->toThrow(\Illuminate\Database\QueryException::class);
});

it('has an embedding', function () {
    $thought = Thought::factory()->create(['embedding' => Embedder::createFakeEmbedding()]);
    expect($thought->embedding->toArray())->toBeArray();
});

test('it may belong to an agent', function () {
    $thought = Thought::factory()->create(['agent_id' => null]);
    expect($thought->agent)->toBeNull();

    $thought = Thought::factory()->create();
    expect($thought->agent)->toBeInstanceOf(Agent::class);
});

test('it may belong to a brain', function () {
    $thought = Thought::factory()->create(['brain_id' => null]);
    expect($thought->brain)->toBeNull();

    $thought = Thought::factory()->create();
    expect($thought->brain)->toBeInstanceOf(Brain::class);
});
