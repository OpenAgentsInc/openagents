<?php

use App\Models\User;
use App\Models\Team;
use App\Models\Thread;
use App\Models\CRM\Activity;
use App\Models\CRM\Contact;
use App\Models\CRM\Company;
use App\Models\CRM\Note;
use App\Models\CRM\Tag;
use Illuminate\Database\QueryException;

beforeEach(function () {
    $this->company = Company::factory()->create();
    $this->user = User::factory()->create();
    $this->contact = Contact::factory()->create([
        'company_id' => $this->company->id,
        'created_by' => $this->user->id,
    ]);
});

test('contact belongs to a company', function () {
    expect($this->contact->company)->toBeInstanceOf(Company::class);
});

test('contact can optionally belong to teams', function () {
    $team = Team::factory()->create();
    $this->contact->teams()->attach($team->id);

    expect($this->contact->teams->first())->toBeInstanceOf(Team::class);
    
    // Test that contact can exist without team
    $contactWithoutTeam = Contact::factory()->create([
        'company_id' => $this->company->id,
        'created_by' => $this->user->id,
    ]);
    
    expect($contactWithoutTeam->teams)->toBeEmpty();
});

test('contact has many activities', function () {
    Activity::factory()->create([
        'contact_id' => $this->contact->id,
        'company_id' => $this->company->id,
        'user_id' => $this->user->id,
    ]);

    expect($this->contact->activities->first())->toBeInstanceOf(Activity::class);
});

test('contact has many chat threads', function () {
    $thread = Thread::factory()->create([
        'user_id' => $this->user->id,
    ]);
    $this->contact->threads()->attach($thread->id);

    expect($this->contact->threads->first())->toBeInstanceOf(Thread::class);
});

test('contact has many notes', function () {
    Note::factory()->create([
        'contact_id' => $this->contact->id,
        'company_id' => $this->company->id,
        'user_id' => $this->user->id,
    ]);

    expect($this->contact->notes->first())->toBeInstanceOf(Note::class);
});

test('contact has many tags', function () {
    $tag = Tag::factory()->create([
        'name' => 'Test Tag',
        'company_id' => $this->company->id,
    ]);
    $this->contact->tags()->attach($tag->id);

    expect($this->contact->tags->first())->toBeInstanceOf(Tag::class);
});

test('contact calculates engagement score', function () {
    Activity::factory()->count(3)->create([
        'contact_id' => $this->contact->id,
        'company_id' => $this->company->id,
        'user_id' => $this->user->id,
    ]);

    $thread = Thread::factory()->create([
        'user_id' => $this->user->id,
    ]);
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
        'company_id' => $this->company->id,
        'created_by' => $this->user->id,
    ]);

    expect($contact->formatted_phone)->toBe('(123) 456-7890');
});

test('contact generates unique contact ids', function () {
    $contact1 = Contact::factory()->create([
        'company_id' => $this->company->id,
        'created_by' => $this->user->id,
    ]);
    $contact2 = Contact::factory()->create([
        'company_id' => $this->company->id,
        'created_by' => $this->user->id,
    ]);

    expect($contact1->contact_id)->not->toBe($contact2->contact_id);
});

test('contact belongs to company and not directly to team', function () {
    $contact = Contact::factory()->create([
        'company_id' => $this->company->id,
        'created_by' => $this->user->id,
    ]);

    expect($contact->company_id)->not->toBeNull()
        ->and($contact->company)->toBeInstanceOf(Company::class);
});