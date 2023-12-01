<?php

use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;
uses(RefreshDatabase::class);

it('can store memory', function() {
    $memory = [
        'title' => 'Test Memory',
        'description' => 'This is a test memory',
        'date' => '2021-01-01',
        'location' => 'Test Location',
        'image' => 'test_image.jpg',
    ];

    $this->post(route('memories.store'), $memory)
        ->assertStatus(201)
        ->assertJson($memory);
});

it('can get all memories', function() {
    $memories = factory(Memory::class, 5)->create();

    $this->get(route('memories.index'))
        ->assertStatus(200)
        ->assertJson($memories->toArray());
});

it('can get memory', function() {
    $memory = factory(Memory::class)->create();

    $this->get(route('memories.show', $memory->id))
        ->assertStatus(200)
        ->assertJson($memory->toArray());
});

it('can update memory', function() {
    $memory = factory(Memory::class)->create();

    $updatedMemory = [
        'title' => 'Updated Memory',
        'description' => 'This is an updated memory',
        'date' => '2021-01-02',
        'location' => 'Updated Location',
        'image' => 'updated_image.jpg',
    ];

    $this->put(route('memories.update', $memory->id), $updatedMemory)
        ->assertStatus(200)
        ->assertJson($updatedMemory);
});

it('can delete memory', function() {
    $memory = factory(Memory::class)->create();

    $this->delete(route('memories.destroy', $memory->id))
        ->assertStatus(204);
});
