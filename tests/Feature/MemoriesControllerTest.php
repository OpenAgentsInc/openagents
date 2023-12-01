<?php

use App\Models\Memory;
use Illuminate\Foundation\Testing\RefreshDatabase;

it('can create a memory from HTTP request', function() {
    $response = $this->postJson('/memories', [
        'description' => 'Test description',
    ]);

    $response->assertOk();
    $this->assertDatabaseHas('memories', [
        'description' => 'Test description',
    ]);
});

it('can read a memory from HTTP request', function() {
    $memory = Memory::create([
        'description' => 'Test description',
        'last_accessed' => null,
    ]);

    $response = $this->getJson("/memories/{$memory->id}");

    $response->assertOk();
    $response->assertJson([
        'description' => 'Test description',
    ]);
});

it('can update a memory from HTTP request', function() {
    $memory = Memory::create([
        'description' => 'Test description',
        'last_accessed' => null,
    ]);

    $response = $this->putJson("/memories/{$memory->id}", [
        'description' => 'Updated description',
    ]);

    $response->assertOk();
    $this->assertDatabaseHas('memories', [
        'description' => 'Updated description',
    ]);
});

it('can delete a memory from HTTP request', function() {
    $memory = Memory::create([
        'description' => 'Test description',
        'last_accessed' => null,
    ]);

    $response = $this->deleteJson("/memories/{$memory->id}");

    $response->assertOk();
    $this->assertDatabaseMissing('memories', ['id' => $memory->id]);
});