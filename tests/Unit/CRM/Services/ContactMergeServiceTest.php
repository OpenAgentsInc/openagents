<?php

namespace Tests\Unit\CRM\Services;

use Tests\TestCase;
use App\Models\Contact;
use App\Models\Activity;
use App\Models\Email;
use App\Models\Note;
use App\Models\Team;
use App\Services\CRM\ContactMergeService;
use Illuminate\Foundation\Testing\RefreshDatabase;

class ContactMergeServiceTest extends TestCase
{
    use RefreshDatabase;

    private ContactMergeService $mergeService;
    private Contact $contact1;
    private Contact $contact2;
    private Team $team;

    protected function setUp(): void
    {
        parent::setUp();
        
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
    }

    /** @test */
    public function it_merges_contact_basic_info()
    {
        $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

        $this->assertEquals('john@example.com', $mergedContact->email);
        $this->assertEquals('1234567890', $mergedContact->phone);
        $this->assertEquals(['john.smith@example.com'], $mergedContact->alternative_emails);
    }

    /** @test */
    public function it_merges_contact_activities()
    {
        Activity::factory()->create(['contact_id' => $this->contact1->id]);
        Activity::factory()->create(['contact_id' => $this->contact2->id]);

        $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

        $this->assertCount(2, $mergedContact->activities);
    }

    /** @test */
    public function it_merges_contact_emails()
    {
        Email::factory()->create(['contact_id' => $this->contact1->id]);
        Email::factory()->create(['contact_id' => $this->contact2->id]);

        $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

        $this->assertCount(2, $mergedContact->emails);
    }

    /** @test */
    public function it_merges_contact_notes()
    {
        Note::factory()->create(['contact_id' => $this->contact1->id]);
        Note::factory()->create(['contact_id' => $this->contact2->id]);

        $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

        $this->assertCount(2, $mergedContact->notes);
    }

    /** @test */
    public function it_handles_conflict_resolution()
    {
        $mergedContact = $this->mergeService->merge(
            $this->contact1,
            $this->contact2,
            ['email' => 'contact2'] // Prefer contact2's email
        );

        $this->assertEquals('john.smith@example.com', $mergedContact->email);
        $this->assertEquals(['john@example.com'], $mergedContact->alternative_emails);
    }

    /** @test */
    public function it_maintains_audit_trail()
    {
        $mergedContact = $this->mergeService->merge($this->contact1, $this->contact2);

        $this->assertNotNull($mergedContact->merge_history);
        $this->assertArrayHasKey($this->contact2->id, $mergedContact->merge_history);
    }
}