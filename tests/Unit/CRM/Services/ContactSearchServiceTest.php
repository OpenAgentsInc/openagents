<?php

use App\Models\Contact;
use App\Models\Team;
use App\Models\User;
use App\Services\CRM\ContactSearchService;

beforeEach(function () {
    $this->team = Team::factory()->create();
    $this->user = User::factory()->create();
    $this->searchService = new ContactSearchService();

    // Create some test contacts
    Contact::factory()->count(20)->create([
        'team_id' => $this->team->id,
    ]);
});

test('search service indexes contact data', function () {
    $contact = Contact::factory()->create([
        'team_id' => $this->team->id,
        'name' => 'John Smith',
        'email' => 'john@example.com',
    ]);

    $indexed = $this->searchService->indexContact($contact);

    expect($indexed)->toBeTrue();
});

test('search service performs fuzzy matching', function () {
    Contact::factory()->create([
        'team_id' => $this->team->id,
        'name' => 'Jonathan Smith',
        'email' => 'jonathan.smith@example.com',
    ]);

    $results = $this->searchService->search('Jon Smith', $this->team->id);

    expect($results)
        ->not->toBeEmpty()
        ->first()->name->toBe('Jonathan Smith');
});

test('search service ranks results', function () {
    Contact::factory()->create([
        'team_id' => $this->team->id,
        'name' => 'John Smith',
        'email' => 'john.smith@example.com',
    ]);

    Contact::factory()->create([
        'team_id' => $this->team->id,
        'name' => 'John Doe',
        'email' => 'john.doe@example.com',
    ]);

    $results = $this->searchService->search('John Smith', $this->team->id);

    expect($results->first()->name)->toBe('John Smith');
});

test('search service filters by permissions', function () {
    $otherTeam = Team::factory()->create();

    Contact::factory()->create([
        'team_id' => $otherTeam->id,
        'name' => 'John Smith',
    ]);

    $results = $this->searchService->search('John Smith', $this->team->id);

    expect($results)->toBeEmpty();
});

test('search service optimizes query performance', function () {
    Contact::factory()->count(100)->create([
        'team_id' => $this->team->id,
    ]);

    $startTime = microtime(true);
    $results = $this->searchService->search('John', $this->team->id);
    $endTime = microtime(true);
    
    $executionTime = ($endTime - $startTime);
    expect($executionTime)->toBeLessThan(0.1);
});

test('search service handles complex criteria', function () {
    Contact::factory()->create([
        'team_id' => $this->team->id,
        'name' => 'John Smith',
        'email' => 'john@techcorp.com',
        'company' => 'Tech Corp',
        'phone' => '1234567890',
    ]);

    $criteria = [
        'name' => 'John',
        'company' => 'Tech',
        'email_domain' => 'techcorp.com',
    ];

    $results = $this->searchService->searchWithCriteria($criteria, $this->team->id);

    expect($results)
        ->not->toBeEmpty()
        ->first()->name->toBe('John Smith');
});

test('search service provides suggestions', function () {
    Contact::factory()->create([
        'team_id' => $this->team->id,
        'name' => 'Jonathan Smith',
    ]);

    $suggestions = $this->searchService->getSuggestions('Jon', $this->team->id);

    expect($suggestions)
        ->not->toBeEmpty()
        ->toContain('Jonathan Smith');
});

test('search service handles empty search gracefully', function () {
    $results = $this->searchService->search('', $this->team->id);

    expect($results)->toBeEmpty();
});