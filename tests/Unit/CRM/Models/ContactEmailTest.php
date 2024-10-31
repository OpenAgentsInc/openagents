<?php

namespace Tests\Unit\CRM\Models;

use Tests\TestCase;
use App\Models\Contact;
use App\Models\Email;
use App\Models\Team;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Storage;

class ContactEmailTest extends TestCase
{
    use RefreshDatabase;

    private Contact $contact;
    private Email $email;

    protected function setUp(): void
    {
        parent::setUp();
        
        $team = Team::factory()->create();
        $this->contact = Contact::factory()->create([
            'team_id' => $team->id,
        ]);
        $this->email = Email::factory()->create([
            'contact_id' => $this->contact->id,
            'subject' => 'Test Email',
            'body' => 'Test content',
            'thread_id' => 'test-thread-123',
        ]);
    }

    /** @test */
    public function it_belongs_to_a_contact()
    {
        $this->assertInstanceOf(Contact::class, $this->email->contact);
    }

    /** @test */
    public function it_tracks_email_metadata()
    {
        $this->assertNotNull($this->email->sent_at);
        $this->assertNotNull($this->email->message_id);
        $this->assertNotNull($this->email->thread_id);
    }

    /** @test */
    public function it_handles_email_threading()
    {
        $reply = Email::factory()->create([
            'contact_id' => $this->contact->id,
            'thread_id' => 'test-thread-123',
            'in_reply_to' => $this->email->message_id,
        ]);

        $this->assertEquals($this->email->thread_id, $reply->thread_id);
        $this->assertEquals($this->email->message_id, $reply->in_reply_to);
    }

    /** @test */
    public function it_manages_email_attachments()
    {
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

        $this->assertTrue($email->has_attachments);
        $this->assertNotNull($email->attachments->first());
        $this->assertEquals('test.pdf', $attachment->filename);
    }

    /** @test */
    public function it_validates_email_addresses()
    {
        $this->expectException(\Illuminate\Database\QueryException::class);
        
        Email::factory()->create([
            'contact_id' => $this->contact->id,
            'from' => 'invalid-email', // Invalid email format
        ]);
    }
}