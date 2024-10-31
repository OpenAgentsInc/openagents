<?php

namespace Tests\Unit\CRM\Services;

use Tests\TestCase;
use App\Models\Contact;
use App\Models\Team;
use App\Models\User;
use App\Services\CRM\ContactSearchService;
use Illuminate\Foundation\Testing\RefreshDatabase;

class ContactSearchServiceTest extends TestCase
{
    use RefreshDatabase;

    private ContactSearchService $searchService;
    private Team $team;
    private User $user;

    protected function setUp(): void
    {
        parent::setUp();
        
        $this->team = Team::factory()->create();
        $this->user = User::factory()->create();
        $this->searchService = new ContactSearchService();

        // Create some test contacts
        Contact::factory()->count(20)->create([
            'team_id' => $this->team->id,
        ]);
    }

    /** @test */
    public function it_indexes_contact_data()
    {
        $contact = Contact::factory()->create([
            'team_id' => $this->team->id,
            'name' => 'John Smith',
            'email' => 'john@example.com',
        ]);

        $indexed = $this->searchService->indexContact($contact);

        $this->assertTrue($indexed);
    }

    /** @test */
    public function it_performs_fuzzy_matching()
    {
        Contact::factory()->create([
            'team_id' => $this->team->id,
            'name' => 'Jonathan Smith',
            'email' => 'jonathan.smith@example.com',
        ]);

        $results = $this->searchService->search('Jon Smith', $this->team->id);

        $this->assertNotEmpty($results);
        $this->assertEquals('Jonathan Smith', $results->first()->name);
    }

    /** @test */
    public function it_ranks_search_results()
    {
        // Create contacts with varying degrees of match
        Contact::factory()->create([
            'team_id' => $this->team->id,
            'name' => 'John Smith',
            'email' => 'john.smith@example.com',
        ]);

        Contact::factory()->create([
            'team_id' => $this->team->id,
            'name' => 'John Doe',
            'email' => 'john.doe@example.com',
        ]);

        $results = $this->searchService->search('John Smith', $this->team->id);

        $this->assertEquals('John Smith', $results->first()->name);
    }

    /** @test */
    public function it_filters_by_permissions()
    {
        $otherTeam = Team::factory()->create();

        Contact::factory()->create([
            'team_id' => $otherTeam->id,
            'name' => 'John Smith',
        ]);

        $results = $this->searchService->search('John Smith', $this->team->id);

        $this->assertEmpty($results);
    }

    /** @test */
    public function it_optimizes_query_performance()
    {
        // Create a large number of contacts
        Contact::factory()->count(100)->create([
            'team_id' => $this->team->id,
        ]);

        $startTime = microtime(true);
        
        $results = $this->searchService->search('John', $this->team->id);
        
        $endTime = microtime(true);
        $executionTime = ($endTime - $startTime);

        // Assert that the search completes within a reasonable time (e.g., 100ms)
        $this->assertLessThan(0.1, $executionTime);
    }

    /** @test */
    public function it_handles_complex_search_criteria()
    {
        Contact::factory()->create([
            'team_id' => $this->team->id,
            'name' => 'John Smith',
            'email' => 'john@techcorp.com',
            'company' => 'Tech Corp',
            'phone' => '1234567890',
        ]);

        $criteria = [
            'name' => 'John',
            'company' => 'Tech',
            'email_domain' => 'techcorp.com',
        ];

        $results = $this->searchService->searchWithCriteria($criteria, $this->team->id);

        $this->assertNotEmpty($results);
        $this->assertEquals('John Smith', $results->first()->name);
    }

    /** @test */
    public function it_provides_search_suggestions()
    {
        Contact::factory()->create([
            'team_id' => $this->team->id,
            'name' => 'Jonathan Smith',
        ]);

        $suggestions = $this->searchService->getSuggestions('Jon', $this->team->id);

        $this->assertNotEmpty($suggestions);
        $this->assertContains('Jonathan Smith', $suggestions);
    }

    /** @test */
    public function it_handles_empty_search_gracefully()
    {
        $results = $this->searchService->search('', $this->team->id);

        $this->assertEmpty($results);
    }
}