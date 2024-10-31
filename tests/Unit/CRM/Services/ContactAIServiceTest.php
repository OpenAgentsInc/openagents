<?php

use App\Models\Contact;
use App\Models\Team;
use App\Models\Activity;
use App\Models\Thread;
use App\Services\CRM\ContactAIService;

beforeEach(function () {
    $this->team = Team::factory()->create();
    $this->contact = Contact::factory()->create([
        'team_id' => $this->team->id,
    ]);
    $this->aiService = new ContactAIService();
});

test('ai service analyzes contact interactions', function () {
    Activity::factory()->count(5)->create([
        'contact_id' => $this->contact->id,
    ]);

    $thread = Thread::factory()->create();
    $this->contact->threads()->attach($thread->id);

    $analysis = $this->aiService->analyzeInteractions($this->contact);

    expect($analysis)
        ->toHaveKey('engagement_level')
        ->toHaveKey('interaction_frequency')
        ->toHaveKey('sentiment_score');
});

test('ai service generates contact summaries', function () {
    Activity::factory()->create([
        'contact_id' => $this->contact->id,
        'type' => 'meeting',
        'description' => 'Discussed new project requirements',
    ]);

    $summary = $this->aiService->generateSummary($this->contact);

    expect($summary)
        ->not->toBeEmpty()
        ->toHaveKey('key_points')
        ->toHaveKey('next_steps');
});

test('ai service calculates relationship scores', function () {
    $scores = $this->aiService->calculateRelationshipScores($this->contact);

    expect($scores)
        ->toHaveKey('overall')
        ->toHaveKey('communication')
        ->toHaveKey('engagement')
        ->toHaveKey('sentiment');
});

test('ai service identifies action items', function () {
    Activity::factory()->create([
        'contact_id' => $this->contact->id,
        'type' => 'email',
        'description' => 'Need to follow up on proposal',
    ]);

    $actionItems = $this->aiService->identifyActionItems($this->contact);

    expect($actionItems)
        ->not->toBeEmpty()
        ->first()->toHaveKeys(['priority', 'description', 'due_date']);
});

test('ai service suggests follow ups', function () {
    Activity::factory()->create([
        'contact_id' => $this->contact->id,
        'type' => 'meeting',
        'description' => 'Initial consultation',
    ]);

    $suggestions = $this->aiService->suggestFollowUps($this->contact);

    expect($suggestions)
        ->not->toBeEmpty()
        ->first()->toHaveKeys(['timing', 'type', 'message']);
});

test('ai service handles missing data gracefully', function () {
    $newContact = Contact::factory()->create([
        'team_id' => $this->team->id,
    ]);

    $analysis = $this->aiService->analyzeInteractions($newContact);

    expect($analysis)
        ->toHaveKey('engagement_level')
        ->engagement_level->toBe('new');
});

test('ai service respects rate limits', function () {
    // Simulate multiple rapid requests
    for ($i = 0; $i < 5; $i++) {
        $this->aiService->analyzeInteractions($this->contact);
    }

    // This should throw a rate limit exception
    $this->aiService->analyzeInteractions($this->contact);
})->throws(Exception::class, 'AI service rate limit exceeded');