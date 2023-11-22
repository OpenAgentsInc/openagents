<?php

use App\Models\Memory;
use Carbon\Carbon;
use Illuminate\Foundation\Testing\RefreshDatabase;

it('can create a memory', function() {
    $memory = Memory::create([
        'description' => 'Test description',
        'last_accessed' => null,
    ]);

    $this->assertDatabaseHas('memories', [
        'description' => 'Test description',
        'last_accessed' => null
    ]);
});

it('can update a memory', function() {
    $memory = Memory::create([
        'description' => 'Test description',
        'last_accessed' => null,
    ]);

    $memory->update([
        'description' => 'Updated description',
        'last_accessed' => Carbon::now(),
    ]);

    $this->assertDatabaseHas('memories', [
        'description' => 'Updated description',
    ]);
});
it('can delete a memory', function() {
    // Create a new memory
    $memory = Memory::create([
        'description' => 'Test description',
        'last_accessed' => null,
    ]);

    // Delete the memory
    $response = $this->delete('/api/memories/' . $memory->id);

    // Verify that the memory was deleted from the database
    $this->assertDatabaseMissing('memories', ['id' => $memory->id]);

    // Verify that the correct status code is returned
    $response->assertStatus(200);
});
