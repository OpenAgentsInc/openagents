<?php

use App\Models\Contact;
use App\Models\Team;
use App\Models\User;
use App\Models\Activity;
use App\Models\Thread;
use App\Models\Note;
use App\Models\Tag;

beforeEach(function () {
    $this->team = Team::factory()->create();
    $this->user = User::factory()->create();
    $this->contact = Contact::factory()->create([
        'team_id' => $this->team->id,
        'created_by' => $this->user->id,
    ]);
});

test('contact belongs to a team', function () {
    expect($this->contact->team)->toBeInstanceOf(Team::class);
});

test('contact has many activities', function () {
    Activity::factory()->create([
        'contact_id' => $this->contact->id,
    ]);

    expect($this->contact->activities->first())->toBeInstanceOf(Activity::class);
});

test('contact has many chat threads', function () {
    $thread = Thread::factory()->create();
    $this->contact->threads()->attach($thread->id);

    expect($this->contact->threads->first())->toBeInstanceOf(Thread::class);
});

test('contact has many notes', function () {
    Note::factory()->create([
        'contact_id' => $this->contact->id,
    ]);

    expect($this->contact->notes->first())->toBeInstanceOf(Note::class);
});

test('contact has many tags', function () {
    $tag = Tag::factory()->create([
        'team_id' => $this->team->id,
    ]);
    $this->contact->tags()->attach($tag->id);

    expect($this->contact->tags->first())->toBeInstanceOf(Tag::class);
});

test('contact calculates engagement score', function () {
    Activity::factory()->count(3)->create([
        'contact_id' => $this->contact->id,
    ]);

    $thread = Thread::factory()->create();
    $this->contact->threads()->attach($thread->id);

    $score = $this->contact->calculateEngagementScore();
    
    expect($score)
        ->toBeFloat()
        ->toBeGreaterThanOrEqual(0);
});

test('contact requires email field', function () {
    Contact::factory()->create(['email' => null]);
})->throws(QueryException::class);

test('contact formats phone numbers', function () {
    $contact = Contact::factory()->create([
        'phone' => '1234567890',
    ]);

    expect($contact->formatted_phone)->toBe('(123) 456-7890');
});

test('contact generates unique contact ids', function () {
    $contact1 = Contact::factory()->create();
    $contact2 = Contact::factory()->create();

    expect($contact1->contact_id)->not->toBe($contact2->contact_id);
});