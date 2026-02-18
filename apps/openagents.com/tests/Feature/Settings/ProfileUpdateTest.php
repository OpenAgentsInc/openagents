<?php

use App\Models\User;
use Illuminate\Support\Facades\DB;

test('profile page is displayed', function () {
    $user = User::factory()->create();

    $response = $this
        ->actingAs($user)
        ->get(route('profile.edit'));

    $response->assertOk();
});

test('profile information can be updated', function () {
    $user = User::factory()->create();

    $response = $this
        ->actingAs($user)
        ->patch('/settings/profile', [
            'name' => 'Updated Name',
        ]);

    $response
        ->assertSessionHasNoErrors()
        ->assertRedirect(route('profile.edit'));

    $user->refresh();

    expect($user->name)->toBe('Updated Name');
});

test('autopilot settings can be created and updated from profile settings page', function () {
    $user = User::factory()->create([
        'name' => 'Chris',
    ]);

    $response = $this
        ->actingAs($user)
        ->patch('/settings/autopilot', [
            'displayName' => 'Chris Autopilot',
            'tagline' => 'Persistent and practical',
            'ownerDisplayName' => 'Chris',
            'personaSummary' => 'Keep it concise and engineering-minded.',
            'autopilotVoice' => 'calm and direct',
            'principlesText' => "Prefer verification over guessing\nAsk before irreversible actions",
        ]);

    $response
        ->assertSessionHasNoErrors()
        ->assertRedirect(route('profile.edit'));

    $autopilot = DB::table('autopilots')
        ->where('owner_user_id', $user->id)
        ->first(['id', 'display_name', 'tagline', 'config_version']);

    expect($autopilot)->not->toBeNull();
    expect((string) $autopilot->display_name)->toBe('Chris Autopilot');
    expect((string) $autopilot->tagline)->toBe('Persistent and practical');
    expect((int) $autopilot->config_version)->toBeGreaterThan(1);

    $profile = DB::table('autopilot_profiles')
        ->where('autopilot_id', $autopilot->id)
        ->first(['owner_display_name', 'persona_summary', 'autopilot_voice', 'principles']);

    expect($profile)->not->toBeNull();
    expect((string) $profile->owner_display_name)->toBe('Chris');
    expect((string) $profile->persona_summary)->toBe('Keep it concise and engineering-minded.');
    expect((string) $profile->autopilot_voice)->toBe('calm and direct');

    $principles = json_decode((string) $profile->principles, true);
    expect($principles)->toBeArray();
    expect($principles)->toContain('Prefer verification over guessing');
    expect($principles)->toContain('Ask before irreversible actions');
});

test('user can delete their account', function () {
    $user = User::factory()->create();

    $response = $this
        ->actingAs($user)
        ->delete(route('profile.destroy'), [
            'password' => 'password',
        ]);

    $response
        ->assertSessionHasNoErrors()
        ->assertRedirect('/');

    $this->assertGuest();
    expect($user->fresh())->toBeNull();
});
