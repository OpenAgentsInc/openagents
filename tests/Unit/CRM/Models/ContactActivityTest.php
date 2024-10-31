<?php

use App\Models\Contact;
use App\Models\Activity;
use App\Models\User;
use App\Models\Team;

beforeEach(function () {
    $this->team = Team::factory()->create();
    $this->user = User::factory()->create();
    $this->contact = Contact::factory()->create([
        'team_id' => $this->team->id,
    ]);
    $this->activity = Activity::factory()->create([
        'contact_id' => $this->contact->id,
        'user_id' => $this->user->id,
        'type' => 'email',
        'description' => 'Test activity',
    ]);
});

test('activity belongs to a contact', function () {
    expect($this->activity->contact)->toBeInstanceOf(Contact::class);
});

test('activity belongs to a user', function () {
    expect($this->activity->user)->toBeInstanceOf(User::class);
});

test('activity has type', function () {
    expect($this->activity->type)->toBe('email');
});

test('activity has timestamp', function () {
    expect($this->activity->created_at)->not->toBeNull();
});

test('activity can have optional notes', function () {
    $activity = Activity::factory()->create([
        'contact_id' => $this->contact->id,
        'user_id' => $this->user->id,
        'notes' => 'Test notes',
    ]);

    expect($activity->notes)->toBe('Test notes');
});

test('activity can link to related content', function () {
    $activity = Activity::factory()->create([
        'contact_id' => $this->contact->id,
        'user_id' => $this->user->id,
        'related_type' => 'thread',
        'related_id' => 1,
    ]);

    expect($activity)
        ->related_type->toBe('thread')
        ->related_id->toBe(1);
});

test('activity requires type field', function () {
    Activity::factory()->create([
        'type' => null,
    ]);
})->throws(QueryException::class);