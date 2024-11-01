<?php

use App\Models\Project;
use App\Models\Thread;
use App\Models\User;
use App\Models\Team;
use App\Models\File;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Illuminate\Validation\ValidationException;

test('a project belongs to a user', function () {
    $project = Project::factory()->forUser()->create();

    expect($project->user)->toBeInstanceOf(User::class);
    expect($project->team)->toBeNull();
});

test('a project belongs to a team', function () {
    $project = Project::factory()->forTeam()->create();

    expect($project->team)->toBeInstanceOf(Team::class);
    expect($project->user)->toBeNull();
});

test('a project has many threads', function () {
    $project = Project::factory()->create();
    $threads = Thread::factory()->count(3)->create(['project_id' => $project->id]);

    expect($project->threads)->toHaveCount(3);
    expect($project->threads->first())->toBeInstanceOf(Thread::class);
});

test('a project belongs to either a user or a team', function () {
    $userProject = Project::factory()->forUser()->create();
    $teamProject = Project::factory()->forTeam()->create();

    expect($userProject->user)->toBeInstanceOf(User::class);
    expect($userProject->team)->toBeNull();

    expect($teamProject->team)->toBeInstanceOf(Team::class);
    expect($teamProject->user)->toBeNull();
});

test('a project has many files', function () {
    $project = Project::factory()->create();
    $files = File::factory()->count(3)->create(['project_id' => $project->id]);

    expect($project->files)->toHaveCount(3);
    expect($project->files->first())->toBeInstanceOf(File::class);
});

test('a file can be uploaded and associated with a project', function () {
    Storage::fake('local');

    $project = Project::factory()->create();
    $file = UploadedFile::fake()->create('document.pdf', 100);

    $response = $this->post('/api/files', [
        'file' => $file,
        'project_id' => $project->id,
    ]);

    $response->assertRedirect();
    $response->assertSessionHas('message', 'File uploaded and ingested.');

    Storage::disk('local')->assertExists('uploads/' . $file->hashName());

    $this->assertDatabaseHas('files', [
        'name' => 'document.pdf',
        'project_id' => $project->id,
    ]);

    $dbFile = File::where('name', 'document.pdf')->first();
    expect($dbFile)->not->toBeNull();
    expect($dbFile->content)->not->toBeEmpty();
})->skip();

// Custom Instructions Tests
test('a project can have custom instructions', function () {
    $project = Project::factory()->create([
        'custom_instructions' => 'Always respond in a formal tone'
    ]);
    expect($project->custom_instructions)->toBe('Always respond in a formal tone');
});

// Project Settings Tests
test('a project can have custom settings', function () {
    $project = Project::factory()->create([
        'settings' => [
            'tone' => 'formal',
            'language' => 'en',
            'role' => 'technical_writer'
        ]
    ]);
    expect($project->settings)->toBeArray();
    expect($project->settings['tone'])->toBe('formal');
});

// Team Access Tests
test('team members can access team project', function () {
    $team = Team::factory()->create();
    $user = User::factory()->create();
    $team->users()->attach($user);
    $project = Project::factory()->forTeam($team)->create();
    
    expect($user->can('view', $project))->toBeTrue();
});

test('non-team members cannot access team project', function () {
    $team = Team::factory()->create();
    $user = User::factory()->create();
    $project = Project::factory()->forTeam($team)->create();
    
    expect($user->can('view', $project))->toBeFalse();
});

// Project Validation Tests
test('project requires a name', function () {
    expect(function () {
        Project::factory()->create(['name' => null]);
    })->toThrow(ValidationException::class);
});

test('project name must be unique within team/user scope', function () {
    $team = Team::factory()->create();
    Project::factory()->forTeam($team)->create(['name' => 'Test Project']);
    
    expect(function () use ($team) {
        Project::factory()->forTeam($team)->create(['name' => 'Test Project']);
    })->toThrow(ValidationException::class);
});

// Thread Context Inheritance Tests
test('threads inherit project context and instructions', function () {
    $project = Project::factory()->create([
        'custom_instructions' => 'Be formal',
        'context' => 'Technical documentation context'
    ]);
    
    $thread = Thread::factory()->create(['project_id' => $project->id]);
    
    expect($thread->getContext())->toContain('Technical documentation context');
    expect($thread->getInstructions())->toContain('Be formal');
});

// Project Archive/Status Tests
test('project can be archived', function () {
    $project = Project::factory()->create();
    $project->archive();
    expect($project->status)->toBe('archived');
});