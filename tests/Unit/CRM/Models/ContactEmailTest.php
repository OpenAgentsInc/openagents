<?php

use App\Models\Contact;
use App\Models\Email;
use App\Models\Team;
use Illuminate\Support\Facades\Storage;

beforeEach(function () {
    $this->team = Team::factory()->create();
    $this->contact = Contact::factory()->create([
        'team_id' => $this->team->id,
    ]);
    $this->email = Email::factory()->create([
        'contact_id' => $this->contact->id,
        'subject' => 'Test Email',
        'body' => 'Test content',
        'thread_id' => 'test-thread-123',
    ]);
});

test('email belongs to a contact', function () {
    expect($this->email->contact)->toBeInstanceOf(Contact::class);
});

test('email tracks metadata', function () {
    expect($this->email)
        ->sent_at->not->toBeNull()
        ->message_id->not->toBeNull()
        ->thread_id->not->toBeNull();
});

test('email handles threading', function () {
    $reply = Email::factory()->create([
        'contact_id' => $this->contact->id,
        'thread_id' => 'test-thread-123',
        'in_reply_to' => $this->email->message_id,
    ]);

    expect($reply)
        ->thread_id->toBe($this->email->thread_id)
        ->in_reply_to->toBe($this->email->message_id);
});

test('email manages attachments', function () {
    Storage::fake('attachments');

    $email = Email::factory()->create([
        'contact_id' => $this->contact->id,
        'has_attachments' => true,
    ]);

    $attachment = $email->attachments()->create([
        'filename' => 'test.pdf',
        'path' => 'attachments/test.pdf',
        'mime_type' => 'application/pdf',
        'size' => 1024,
    ]);

    expect($email->has_attachments)->toBeTrue()
        ->and($email->attachments->first())->not->toBeNull()
        ->and($attachment->filename)->toBe('test.pdf');
});

test('email validates addresses', function () {
    Email::factory()->create([
        'contact_id' => $this->contact->id,
        'from' => 'invalid-email',
    ]);
})->throws(QueryException::class);