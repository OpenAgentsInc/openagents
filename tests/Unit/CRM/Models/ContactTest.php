<?php

namespace Tests\Unit\CRM\Models;

use Tests\TestCase;
use App\Models\Contact;
use App\Models\Team;
use App\Models\User;
use App\Models\Activity;
use App\Models\Thread;
use App\Models\Note;
use App\Models\Tag;
use Illuminate\Foundation\Testing\RefreshDatabase;

class ContactTest extends TestCase
{
    use RefreshDatabase;

    private Contact $contact;
    private Team $team;
    private User $user;

    protected function setUp(): void
    {
        parent::setUp();
        
        $this->team = Team::factory()->create();
        $this->user = User::factory()->create();
        $this->contact = Contact::factory()->create([
            'team_id' => $this->team->id,
            'created_by' => $this->user->id,
        ]);
    }

    /** @test */
    public function it_belongs_to_a_team()
    {
        $this->assertInstanceOf(Team::class, $this->contact->team);
    }

    /** @test */
    public function it_has_many_activities()
    {
        Activity::factory()->create([
            'contact_id' => $this->contact->id,
        ]);

        $this->assertInstanceOf(Activity::class, $this->contact->activities->first());
    }

    /** @test */
    public function it_has_many_chat_threads()
    {
        $thread = Thread::factory()->create();
        $this->contact->threads()->attach($thread->id);

        $this->assertInstanceOf(Thread::class, $this->contact->threads->first());
    }

    /** @test */
    public function it_has_many_notes()
    {
        Note::factory()->create([
            'contact_id' => $this->contact->id,
        ]);

        $this->assertInstanceOf(Note::class, $this->contact->notes->first());
    }

    /** @test */
    public function it_has_many_tags()
    {
        $tag = Tag::factory()->create([
            'team_id' => $this->team->id,
        ]);
        $this->contact->tags()->attach($tag->id);

        $this->assertInstanceOf(Tag::class, $this->contact->tags->first());
    }

    /** @test */
    public function it_calculates_engagement_score()
    {
        // Create some activities
        Activity::factory()->count(3)->create([
            'contact_id' => $this->contact->id,
        ]);

        // Create some threads
        $thread = Thread::factory()->create();
        $this->contact->threads()->attach($thread->id);

        $score = $this->contact->calculateEngagementScore();
        $this->assertIsFloat($score);
        $this->assertGreaterThanOrEqual(0, $score);
    }

    /** @test */
    public function it_validates_required_fields()
    {
        $this->expectException(\Illuminate\Database\QueryException::class);
        
        Contact::factory()->create([
            'email' => null,
        ]);
    }

    /** @test */
    public function it_formats_phone_numbers()
    {
        $contact = Contact::factory()->create([
            'phone' => '1234567890',
        ]);

        $this->assertEquals('(123) 456-7890', $contact->formatted_phone);
    }

    /** @test */
    public function it_generates_unique_contact_ids()
    {
        $contact1 = Contact::factory()->create();
        $contact2 = Contact::factory()->create();

        $this->assertNotEquals($contact1->contact_id, $contact2->contact_id);
    }
}