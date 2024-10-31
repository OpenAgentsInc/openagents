   FAIL  Tests\Feature\ThreadOwnershipTest
  ⨯ threads are created without project when no team is selected                                       0.30s
  ⨯ threads are created with default project when team is selected                                     0.06s
  ✓ user can view their personal threads                                                               0.03s
  ✓ user can view their team threads                                                                   0.01s
  ✓ user cannot view threads from teams they dont belong to                                            0.01s
  ✓ user cannot view other users personal threads                                                      0.01s
  ✓ threads list shows only personal threads in personal context                                       0.01s
  ✓ threads list shows only team threads in team context                                               0.01s
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\ThreadOwnershipTest > threads are created without project when no team is selecte…
  Expected response status code [201, 301, 302, 303, 307, 308] but received 405.
Failed asserting that false is true.

  at tests/Feature/ThreadOwnershipTest.php:15
     11▕
     12▕     $response = $this->actingAs($user)
     13▕         ->post(route('chat.create'));
     14▕
  ➜  15▕     $response->assertRedirect();
     16▕
     17▕     $thread = Thread::where('user_id', $user->id)->latest()->first();
     18▕
     19▕     expect($thread)->not->toBeNull()

  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\ThreadOwnershipTest > threads are created with default project when team is selec…
  Expected response status code [201, 301, 302, 303, 307, 308] but received 405.
Failed asserting that false is true.

  at tests/Feature/ThreadOwnershipTest.php:34
     30▕
     31▕     $response = $this->actingAs($user)
     32▕         ->post(route('chat.create'));
     33▕
  ➜  34▕     $response->assertRedirect();
     35▕
     36▕     $thread = Thread::where('user_id', $user->id)->latest()->first();
     37▕     $project = Project::where('team_id', $team->id)
     38▕         ->where('is_default', true)


  Tests:    2 failed, 6 passed (10 assertions)
  Duration: 0.52s
