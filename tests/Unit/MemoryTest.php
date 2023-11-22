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
    $memory = Memory::create([
        'description' => 'Test description',
        'last_accessed' => null,
    ]);

    $this->assertDatabaseHas('memories', [
        'description' => 'Test description',
        'last_accessed' => null
    ]);
    
    $memory->delete();
    
    $this->assertDatabaseMissing('memories', ['id' => $memory->id]);
});
```

Explanation: The `assertDatabaseHas` method should be called before the `delete` method to ensure that the record exists in the database before attempting to delete it. This ensures that the test is checking for the correct behavior and avoids any potential errors.
