<?php

use App\Models\Contact;
use App\Models\Activity;
use App\Models\Email;
use App\Models\Note;
use App\Models\Team;
use App\Services\CRM\ContactMergeService;

beforeEach(function () {
    $this->team = Team::factory()->create();
    $this->contact1 = Contact::factory()->create([
        'team_id' => $this->team->id,
        'email' => 'john@example.com',
        'phone' => '1234567890',
    ]);
    $this->contact2 = Contact::factory()->create([
        'team_id' => $this->team->id,
        'email' => 'john.smith@example.com',
        'phone' => '0987654321',
    ]);
    
    $this->mergeService = new ContactMergeService();
});

test('merge service combines contact basic info', function () {
    $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

    expect($mergedContact)
        ->email->toBe('john@example.com')
        ->phone->toBe('1234567890')
        ->alternative_emails->toBe(['john.smith@example.com']);
});

test('merge service combines activities', function () {
    Activity::factory()->create(['contact_id' => $this->contact1->id]);
    Activity::factory()->create(['contact_id' => $this->contact2->id]);

    $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

    expect($mergedContact->activities)->toHaveCount(2);
});

test('merge service combines emails', function () {
    Email::factory()->create(['contact_id' => $this->contact1->id]);
    Email::factory()->create(['contact_id' => $this->contact2->id]);

    $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

    expect($mergedContact->emails)->toHaveCount(2);
});

test('merge service combines notes', function () {
    Note::factory()->create(['contact_id' => $this->contact1->id]);
    Note::factory()->create(['contact_id' => $this->contact2->id]);

    $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

    expect($mergedContact->notes)->toHaveCount(2);
});

test('merge service handles conflict resolution', function () {
    $mergedContact = $this->mergeService->merge(
        $this->contact1,
        $this->contact2,
        ['email' => 'contact2'] // Prefer contact2's email
    );

    expect($mergedContact)
        ->email->toBe('john.smith@example.com')
        ->alternative_emails->toBe(['john@example.com']);
});

test('merge service maintains audit trail', function () {
    $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

    expect($mergedContact->merge_history)
        ->not->toBeNull()
        ->toHaveKey($this->contact2->id);
});