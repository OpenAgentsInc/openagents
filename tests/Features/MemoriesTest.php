<?php

use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

uses(RefreshDatabase::class);

it('can create a new Memory object', function () {
    $memory = Memory::factory()->create();

    $this->post('/memories', $memory->toArray());

    $this->assertDatabaseHas('memories', $memory->toArray());
});

it('can retrieve a Memory object by ID', function () {
    $memory = Memory::factory()->create();

    $this->get('/memories/' . $memory->id);

    $this->assertEquals($memory->toArray(), $this->response->json());
});

it('can update a Memory object', function () {
    $memory = Memory::factory()->create();

    $this->put('/memories/' . $memory->id, ['title' => 'Updated Title']);

    $this->assertDatabaseHas('memories', ['id' => $memory->id, 'title' => 'Updated Title']);
});

it('can delete a Memory object', function () {
    $memory = Memory::factory()->create();

    $this->delete('/memories/' . $memory->id);

    $this->assertDatabaseMissing('memories', ['id' => $memory->id]);
});