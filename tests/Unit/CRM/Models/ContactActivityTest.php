<?php

namespace Tests\Unit\CRM\Models;

use Tests\TestCase;
use App\Models\Contact;
use App\Models\Activity;
use App\Models\User;
use App\Models\Team;
use Illuminate\Foundation\Testing\RefreshDatabase;

class ContactActivityTest extends TestCase
{
    use RefreshDatabase;

    private Activity $activity;
    private Contact $contact;
    private User $user;

    protected function setUp(): void
    {
        parent::setUp();
        
        $team = Team::factory()->create();
        $this->user = User::factory()->create();
        $this->contact = Contact::factory()->create([
            'team_id' => $team->id,
        ]);
        $this->activity = Activity::factory()->create([
            'contact_id' => $this->contact->id,
            'user_id' => $this->user->id,
            'type' => 'email',
            'description' => 'Test activity',
        ]);
    }

    /** @test */
    public function it_belongs_to_a_contact()
    {
        $this->assertInstanceOf(Contact::class, $this->activity->contact);
    }

    /** @test */
    public function it_belongs_to_a_user()
    {
        $this->assertInstanceOf(User::class, $this->activity->user);
    }

    /** @test */
    public function it_has_activity_type()
    {
        $this->assertEquals('email', $this->activity->type);
    }

    /** @test */
    public function it_has_timestamp()
    {
        $this->assertNotNull($this->activity->created_at);
    }

    /** @test */
    public function it_has_optional_notes()
    {
        $activity = Activity::factory()->create([
            'contact_id' => $this->contact->id,
            'user_id' => $this->user->id,
            'notes' => 'Test notes',
        ]);

        $this->assertEquals('Test notes', $activity->notes);
    }

    /** @test */
    public function it_links_to_related_content()
    {
        $activity = Activity::factory()->create([
            'contact_id' => $this->contact->id,
            'user_id' => $this->user->id,
            'related_type' => 'thread',
            'related_id' => 1,
        ]);

        $this->assertEquals('thread', $activity->related_type);
        $this->assertEquals(1, $activity->related_id);
    }

    /** @test */
    public function it_validates_activity_data()
    {
        $this->expectException(\Illuminate\Database\QueryException::class);
        
        Activity::factory()->create([
            'type' => null,
        ]);
    }
}