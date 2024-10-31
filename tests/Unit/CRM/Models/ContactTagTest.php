<?php

namespace Tests\Unit\CRM\Models;

use Tests\TestCase;
use App\Models\Contact;
use App\Models\Tag;
use App\Models\Team;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Database\QueryException;

class ContactTagTest extends TestCase
{
    use RefreshDatabase;

    private Contact $contact;
    private Tag $tag;
    private Team $team;

    protected function setUp(): void
    {
        parent::setUp();
        
        $this->team = Team::factory()->create();
        $this->contact = Contact::factory()->create([
            'team_id' => $this->team->id,
        ]);
        $this->tag = Tag::factory()->create([
            'team_id' => $this->team->id,
            'name' => 'test-tag',
        ]);
    }

    /** @test */
    public function it_belongs_to_a_contact()
    {
        $this->contact->tags()->attach($this->tag->id);
        $this->assertInstanceOf(Tag::class, $this->contact->tags->first());
    }

    /** @test */
    public function it_belongs_to_a_team()
    {
        $this->assertInstanceOf(Team::class, $this->tag->team);
    }

    /** @test */
    public function it_has_unique_constraints()
    {
        $this->expectException(QueryException::class);
        
        // Try to create duplicate tag
        Tag::factory()->create([
            'team_id' => $this->team->id,
            'name' => 'test-tag',
        ]);
    }

    /** @test */
    public function it_validates_tag_format()
    {
        $this->expectException(QueryException::class);
        
        Tag::factory()->create([
            'team_id' => $this->team->id,
            'name' => '', // Empty tag name should fail
        ]);
    }
}