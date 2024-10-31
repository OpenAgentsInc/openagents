<?php

use App\Models\Team;
use App\Models\Contact;
use App\Services\CRM\ContactImportService;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;

beforeEach(function () {
    $this->team = Team::factory()->create();
    $this->importService = new ContactImportService();
});

test('import service validates file format', function () {
    Storage::fake('imports');
    $file = UploadedFile::fake()->create('invalid.txt', 100);
    
    $this->importService->import($file, $this->team->id);
})->throws(InvalidArgumentException::class);

test('import service maps fields correctly', function () {
    Storage::fake('imports');

    $csvContent = "Name,Email,Phone\nJohn Doe,john@example.com,1234567890";
    $file = UploadedFile::fake()->createWithContent('contacts.csv', $csvContent);

    $mapping = [
        'name' => 'Name',
        'email' => 'Email',
        'phone' => 'Phone',
    ];

    $result = $this->importService->import($file, $this->team->id, $mapping);

    expect($result->successful)->toBeTrue();
    
    expect(Contact::first())
        ->name->toBe('John Doe')
        ->email->toBe('john@example.com')
        ->phone->toBe('1234567890');
});

test('import service detects duplicates', function () {
    Storage::fake('imports');

    // Create existing contact
    Contact::factory()->create([
        'team_id' => $this->team->id,
        'email' => 'john@example.com',
    ]);

    $csvContent = "Name,Email,Phone\nJohn Doe,john@example.com,1234567890";
    $file = UploadedFile::fake()->createWithContent('contacts.csv', $csvContent);

    $result = $this->importService->import($file, $this->team->id);

    expect($result)
        ->successful->toBeTrue()
        ->duplicates->toHaveCount(1);
});

test('import service processes batch imports', function () {
    Storage::fake('imports');

    $csvContent = "Name,Email,Phone\n" . 
        implode("\n", array_map(function($i) {
            return "User $i,user$i@example.com,$i";
        }, range(1, 100)));

    $file = UploadedFile::fake()->createWithContent('contacts.csv', $csvContent);

    $result = $this->importService->import($file, $this->team->id);

    expect($result)
        ->successful->toBeTrue()
        ->imported->toBe(100);
});

test('import service reports errors', function () {
    Storage::fake('imports');

    $csvContent = "Name,Email,Phone\nJohn Doe,invalid-email,1234567890";
    $file = UploadedFile::fake()->createWithContent('contacts.csv', $csvContent);

    $result = $this->importService->import($file, $this->team->id);

    expect($result)
        ->hasErrors()->toBeTrue()
        ->errors->toHaveCount(1)
        ->errors->first()->message->toBe('Invalid email format');
});