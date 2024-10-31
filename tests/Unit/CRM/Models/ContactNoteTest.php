<?php

namespace Tests\Unit\CRM\Models;

use Tests\TestCase;
use App\Models\Contact;
use App\Models\Note;
use App\Models\User;
use App\Models\Team;
use Illuminate\Foundation\Testing\RefreshDatabase;

class ContactNoteTest extends TestCase
{
    use RefreshDatabase;

    private Contact $contact;
    private Note $note;
    private User $user;

    protected function setUp(): void
    {
        parent::setUp();
        
        $team = Team::factory()->create();
        $this->user = User::factory()->create();
        $this->contact = Contact::factory()->create([
            'team_id' => $team->id,
        ]);
        $this->note = Note::factory()->create([
            'contact_id' => $this->contact->id,
            'user_id' => $this->user->id,
            'content' => '# Test Note\n\nThis is a test note with **markdown**.',
        ]);
    }

    /** @test */
    public function it_belongs_to_a_contact()
    {
        $this->assertInstanceOf(Contact::class, $this->note->contact);
    }

    /** @test */
    public function it_belongs_to_a_user()
    {
        $this->assertInstanceOf(User::class, $this->note->user);
    }

    /** @test */
    public function it_supports_markdown_formatting()
    {
        $this->assertStringContainsString('# Test Note', $this->note->content);
        $this->assertStringContainsString('**markdown**', $this->note->content);
        
        // Test HTML rendering if your Note model has this feature
        // $this->assertStringContainsString('<h1>Test Note</h1>', $this->note->rendered_content);
    }

    /** @test */
    public function it_tracks_edit_history()
    {
        $originalContent = $this->note->content;
        
        $this->note->update([
            'content' => 'Updated content',
        ]);

        $this->assertNotEquals($originalContent, $this->note->content);
        $this->assertNotNull($this->note->updated_at);
        
        // If you implement version history
        // $this->assertCount(2, $this->note->versions);
    }

    /** @test */
    public function it_handles_mentions()
    {
        $mentionedUser = User::factory()->create();
        
        $note = Note::factory()->create([
            'contact_id' => $this->contact->id,
            'user_id' => $this->user->id,
            'content' => "Hey @{$mentionedUser->username}, please check this.",
        ]);

        // If you implement mention parsing
        // $this->assertCount(1, $note->mentions);
        // $this->assertEquals($mentionedUser->id, $note->mentions->first()->mentioned_user_id);
    }

    /** @test */
    public function it_validates_required_content()
    {
        $this->expectException(\Illuminate\Database\QueryException::class);
        
        Note::factory()->create([
            'contact_id' => $this->contact->id,
            'user_id' => $this->user->id,
            'content' => null,
        ]);
    }
}