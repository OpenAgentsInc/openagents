<?php

use App\Models\Memory;
use Illuminate\Foundation\Testing\RefreshDatabase;

it('can fetch all memories', function() {
    Memory::factory()->count(3)->create();

    $response = $this->get('/api/memories');

    $response->assertStatus(200);
});

it('can fetch a single memory', function() {
    $memory = Memory::factory()->create();

    $response = $this->get('/api/memories/' . $memory->id);

    $response->assertStatus(200);
});

it('can create a new memory', function() {
    $data = [
        'description' => 'Test description',
        'last_accessed' => null,
    ];

    $response = $this->post('/api/memories', $data);

    $response->assertStatus(201);
});

it('can update an existing memory', function() {
    $memory = Memory::factory()->create();

    $data = [
        'description' => 'Updated description',
        'last_accessed' => Carbon::now(),
    ];

    $response = $this->put('/api/memories/' . $memory->id, data);

    // Check if the database has been updated
    expect($memory->fresh()->description)->toEqual($data['description']);

});

it('can delete a memory', function() {
  	$memory = Memory::factory()->create();

  	$response = this->delete('/api/memories/' .