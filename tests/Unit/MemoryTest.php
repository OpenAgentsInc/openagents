<?php

use App\Models\Memory;
use Carbon\Carbon;
use Illuminate\Foundation\Testing\RefreshDatabase;
it('can create a memory', function() {
  // Create a new Memory object
  $memory = new Memory();
  
  // Set the attributes for the object
  $memory->description = "Test description";
  $memory->last_accessed = null;
  
  // Save the object to the database
  $memory->save();
  
  // Check if the object was correctly saved into the database
  $this->assertDatabaseHas('memories', [
      'description' => "Test description",
      "last_accessed" => null,
  ]);
});

it('can update a memory', function() {
  // Create a new Memory object
  $memory = new Memory();
  
  // Set the attributes for the object
  $memory->description = "Test description";
  $memory->last_accessed = null;
  
  // Save the object to the database
  $memory->save();
  
  // Update the attributes of the object
  $memory->description = "Updated description";
  $memory->last_accessed = Carbon::now();
  
  // Save the updated object to the database
  $memory->save();
  
  // Check if the object was correctly updated in the database
  $this->assertDatabaseHas('memories', [
      'description' => "Updated description",
      "last_accessed" => Carbon::now(),
  ]);
});

it('can delete a memory', function() {
    // Create a new Memory object
    $memory = new Memory();
    
    // Set the attributes for the object
    $memory->description = "Test description";
    $memory->last_accessed = null;
    
    // Save the object to the database
    $memory->save();
    
    // Delete the object from the database
    $memory->delete();

    // Check if the object is no longer present in the database
    $this->assertDatabaseMissing('memories', ['id' => $memory->id]);
});
