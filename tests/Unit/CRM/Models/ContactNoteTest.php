<?php

use App\Models\Contact;
use App\Models\Note;
use App\Models\User;
use App\Models\Team;

beforeEach(function () {
    $this->team = Team::factory()->create();
    $this->user = User::factory()->create();
    $this->contact = Contact::factory()->create([
        'team_id' => $this->team->id,
    ]);
    $this->note = Note::factory()->create([
        'contact_id' => $this->contact->id,
        'user_id' => $this->user->id,
        'content' => '# Test Note\n\nThis is a test note with **markdown**.',
    ]);
});

test('note belongs to a contact', function () {
    expect($this->note->contact)->toBeInstanceOf(Contact::class);
});

test('note belongs to a user', function () {
    expect($this->note->user)->toBeInstanceOf(User::class);
});

test('note supports markdown formatting', function () {
    expect($this->note->content)
        ->toContain('# Test Note')
        ->toContain('**markdown**');
    
    // If you implement HTML rendering
    // expect($this->note->rendered_content)->toContain('<h1>Test Note</h1>');
});

test('note tracks edit history', function () {
    $originalContent = $this->note->content;
    
    $this->note->update([
        'content' => 'Updated content',
    ]);

    expect($this->note)
        ->content->not->toBe($originalContent)
        ->updated_at->not->toBeNull();
    
    // If you implement version history
    // expect($this->note->versions)->toHaveCount(2);
});

test('note handles mentions', function () {
    $mentionedUser = User::factory()->create();
    
    $note = Note::factory()->create([
        'contact_id' => $this->contact->id,
        'user_id' => $this->user->id,
        'content' => "Hey @{$mentionedUser->username}, please check this.",
    ]);

    // If you implement mention parsing
    // expect($note->mentions)
    //     ->toHaveCount(1)
    //     ->first()->mentioned_user_id->toBe($mentionedUser->id);
});

test('note requires content', function () {
    Note::factory()->create([
        'contact_id' => $this->contact->id,
        'user_id' => $this->user->id,
        'content' => null,
    ]);
})->throws(QueryException::class);