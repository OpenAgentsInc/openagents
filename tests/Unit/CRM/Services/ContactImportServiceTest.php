<?php

namespace Tests\Unit\CRM\Services;

use Tests\TestCase;
use App\Models\Team;
use App\Services\CRM\ContactImportService;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;

class ContactImportServiceTest extends TestCase
{
    use RefreshDatabase;

    private ContactImportService $importService;
    private Team $team;

    protected function setUp(): void
    {
        parent::setUp();
        
        $this->team = Team::factory()->create();
        $this->importService = new ContactImportService();
    }

    /** @test */
    public function it_validates_import_format()
    {
        Storage::fake('imports');

        $file = UploadedFile::fake()->create('invalid.txt', 100);
        
        $this->expectException(\InvalidArgumentException::class);
        
        $this->importService->import($file, $this->team->id);
    }

    /** @test */
    public function it_maps_import_fields()
    {
        Storage::fake('imports');

        $csvContent = "Name,Email,Phone\nJohn Doe,john@example.com,1234567890";
        $file = UploadedFile::fake()->createWithContent('contacts.csv', $csvContent);

        $mapping = [
            'name' => 'Name',
            'email' => 'Email',
            'phone' => 'Phone',
        ];

        $result = $this->importService->import($file, $this->team->id, $mapping);

        $this->assertTrue($result->successful);
        $this->assertDatabaseHas('contacts', [
            'name' => 'John Doe',
            'email' => 'john@example.com',
            'phone' => '1234567890',
        ]);
    }

    /** @test */
    public function it_handles_duplicate_detection()
    {
        Storage::fake('imports');

        // Create existing contact
        $existingContact = Contact::factory()->create([
            'team_id' => $this->team->id,
            'email' => 'john@example.com',
        ]);

        $csvContent = "Name,Email,Phone\nJohn Doe,john@example.com,1234567890";
        $file = UploadedFile::fake()->createWithContent('contacts.csv', $csvContent);

        $result = $this->importService->import($file, $this->team->id);

        $this->assertTrue($result->successful);
        $this->assertCount(1, $result->duplicates);
    }

    /** @test */
    public function it_processes_batch_imports()
    {
        Storage::fake('imports');

        $csvContent = "Name,Email,Phone\n" . 
            implode("\n", array_map(function($i) {
                return "User $i,user$i@example.com,$i";
            }, range(1, 100)));

        $file = UploadedFile::fake()->createWithContent('contacts.csv', $csvContent);

        $result = $this->importService->import($file, $this->team->id);

        $this->assertTrue($result->successful);
        $this->assertEquals(100, $result->imported);
    }

    /** @test */
    public function it_reports_import_errors()
    {
        Storage::fake('imports');

        $csvContent = "Name,Email,Phone\nJohn Doe,invalid-email,1234567890";
        $file = UploadedFile::fake()->createWithContent('contacts.csv', $csvContent);

        $result = $this->importService->import($file, $this->team->id);

        $this->assertTrue($result->hasErrors());
        $this->assertCount(1, $result->errors);
        $this->assertEquals('Invalid email format', $result->errors[0]['message']);
    }
}