 FAIL  Tests\Feature\ThreadOwnershipTest
  ⨯ threads are created without project when no team is selected                                       0.34s
  ⨯ threads are created with default project when team is selected                                     0.07s
  ✓ user can view their personal threads                                                               0.03s
  ✓ user can view their team threads                                                                   0.01s
  ✓ user cannot view threads from teams they dont belong to                                            0.01s
  ✓ user cannot view other users personal threads                                                      0.01s
  ✓ threads list shows only personal threads in personal context                                       0.01s
  ✓ threads list shows only team threads in team context                                               0.01s
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\ThreadOwnershipTest > threads are created without project when no…  ErrorException
  Attempt to read property "user_id" on null

  at tests/Feature/ThreadOwnershipTest.php:16
     12▕         ->post(route('chat.create'));
     13▕
     14▕     $thread = Thread::latest()->first();
     15▕
  ➜  16▕     expect($thread->user_id)->toBe($user->id)
     17▕         ->and($thread->project_id)->toBeNull();
     18▕ });
     19▕
     20▕ test('threads are created with default project when team is selected', function () {

  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\ThreadOwnershipTest > threads are created with default project wh…  ErrorException
  Attempt to read property "user_id" on null

  at tests/Feature/ThreadOwnershipTest.php:35
     31▕     $project = Project::where('team_id', $team->id)
     32▕         ->where('is_default', true)
     33▕         ->first();
     34▕
  ➜  35▕     expect($thread->user_id)->toBe($user->id)
     36▕         ->and($thread->project_id)->toBe($project->id)
     37▕         ->and($project->team_id)->toBe($team->id);
     38▕ });
     39▕


  Tests:    2 failed, 6 passed (8 assertions)
  Duration: 0.59s
