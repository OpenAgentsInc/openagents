<?php

use App\Models\CRM\Contact;
use App\Models\CRM\Tag;
use App\Models\Team;
use Illuminate\Database\QueryException;

beforeEach(function () {
    $this->team = Team::factory()->create();
    $this->contact = Contact::factory()->create([
        'team_id' => $this->team->id,
    ]);
    $this->tag = Tag::factory()->create([
        'team_id' => $this->team->id,
        'name' => 'test-tag',
    ]);
});

test('tag belongs to a contact', function () {
    $this->contact->tags()->attach($this->tag->id);
    expect($this->contact->tags->first())->toBeInstanceOf(Tag::class);
});

test('tag belongs to a team', function () {
    expect($this->tag->team)->toBeInstanceOf(Team::class);
});

test('tag enforces unique constraints', function () {
    Tag::factory()->create([
        'team_id' => $this->team->id,
        'name' => 'test-tag',
    ]);
})->throws(QueryException::class);

test('tag validates format', function () {
    Tag::factory()->create([
        'team_id' => $this->team->id,
        'name' => '', // Empty tag name should fail
    ]);
})->throws(QueryException::class);
