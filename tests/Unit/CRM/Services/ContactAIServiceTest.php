<?php

namespace Tests\Unit\CRM\Services;

use Tests\TestCase;
use App\Models\Contact;
use App\Models\Team;
use App\Models\Activity;
use App\Models\Thread;
use App\Services\CRM\ContactAIService;
use Illuminate\Foundation\Testing\RefreshDatabase;

class ContactAIServiceTest extends TestCase
{
    use RefreshDatabase;

    private ContactAIService $aiService;
    private Contact $contact;
    private Team $team;

    protected function setUp(): void
    {
        parent::setUp();
        
        $this->team = Team::factory()->create();
        $this->contact = Contact::factory()->create([
            'team_id' => $this->team->id,
        ]);
        $this->aiService = new ContactAIService();
    }

    /** @test */
    public function it_analyzes_contact_interactions()
    {
        // Create some activities
        Activity::factory()->count(5)->create([
            'contact_id' => $this->contact->id,
        ]);

        // Create some chat threads
        $thread = Thread::factory()->create();
        $this->contact->threads()->attach($thread->id);

        $analysis = $this->aiService->analyzeInteractions($this->contact);

        $this->assertArrayHasKey('engagement_level', $analysis);
        $this->assertArrayHasKey('interaction_frequency', $analysis);
        $this->assertArrayHasKey('sentiment_score', $analysis);
    }

    /** @test */
    public function it_generates_contact_summaries()
    {
        Activity::factory()->create([
            'contact_id' => $this->contact->id,
            'type' => 'meeting',
            'description' => 'Discussed new project requirements',
        ]);

        $summary = $this->aiService->generateSummary($this->contact);

        $this->assertNotEmpty($summary);
        $this->assertArrayHasKey('key_points', $summary);
        $this->assertArrayHasKey('next_steps', $summary);
    }

    /** @test */
    public function it_calculates_relationship_scores()
    {
        $scores = $this->aiService->calculateRelationshipScores($this->contact);

        $this->assertArrayHasKey('overall', $scores);
        $this->assertArrayHasKey('communication', $scores);
        $this->assertArrayHasKey('engagement', $scores);
        $this->assertArrayHasKey('sentiment', $scores);
    }

    /** @test */
    public function it_identifies_action_items()
    {
        Activity::factory()->create([
            'contact_id' => $this->contact->id,
            'type' => 'email',
            'description' => 'Need to follow up on proposal',
        ]);

        $actionItems = $this->aiService->identifyActionItems($this->contact);

        $this->assertNotEmpty($actionItems);
        $this->assertArrayHasKey('priority', $actionItems[0]);
        $this->assertArrayHasKey('description', $actionItems[0]);
        $this->assertArrayHasKey('due_date', $actionItems[0]);
    }

    /** @test */
    public function it_suggests_follow_ups()
    {
        $lastActivity = Activity::factory()->create([
            'contact_id' => $this->contact->id,
            'type' => 'meeting',
            'description' => 'Initial consultation',
        ]);

        $suggestions = $this->aiService->suggestFollowUps($this->contact);

        $this->assertNotEmpty($suggestions);
        $this->assertArrayHasKey('timing', $suggestions[0]);
        $this->assertArrayHasKey('type', $suggestions[0]);
        $this->assertArrayHasKey('message', $suggestions[0]);
    }

    /** @test */
    public function it_handles_missing_data_gracefully()
    {
        // Contact with no activities
        $newContact = Contact::factory()->create([
            'team_id' => $this->team->id,
        ]);

        $analysis = $this->aiService->analyzeInteractions($newContact);

        $this->assertArrayHasKey('engagement_level', $analysis);
        $this->assertEquals('new', $analysis['engagement_level']);
    }

    /** @test */
    public function it_respects_rate_limits()
    {
        // Simulate multiple rapid requests
        for ($i = 0; $i < 5; $i++) {
            $this->aiService->analyzeInteractions($this->contact);
        }

        // This should throw a rate limit exception
        $this->expectException(\Exception::class);
        $this->expectExceptionMessage('AI service rate limit exceeded');
        
        $this->aiService->analyzeInteractions($this->contact);
    }
}