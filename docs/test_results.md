  FAIL  Tests\Unit\ProjectTest
  ✓ a project belongs to a user                                                                        0.23s
  ✓ a project belongs to a team                                                                        0.01s
  ✓ a project has many threads                                                                         0.01s
  ✓ a project belongs to either a user or a team                                                       0.01s
  ✓ a project has many files                                                                           0.01s
  - a file can be uploaded and associated with a project                                               0.01s
  ✓ a project can have custom instructions                                                             0.01s
  ✓ a project can have custom settings
  ⨯ team members can access team project                                                               0.01s
  ✓ non-team members cannot access team project
  ✓ project requires a name                                                                            0.01s
  ✓ project name must be unique within team/user scope                                                 0.01s
  ⨯ threads inherit project context and instructions
  ✓ project can be archived
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\ProjectTest > team members can access team project
  Failed asserting that false is true.

  at tests/Unit/ProjectTest.php:107
    103▕     $user = User::factory()->create();
    104▕     $team->users()->attach($user);
    105▕     $project = Project::factory()->forTeam($team)->create();
    106▕
  ➜ 107▕     expect($user->can('view', $project))->toBeTrue();
    108▕ });
    109▕
    110▕ test('non-team members cannot access team project', function () {
    111▕     $team = Team::factory()->create();

  1   tests/Unit/ProjectTest.php:107

  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\ProjectTest > threads inherit project context and instructio…  BadMethodCallException
  Call to undefined method App\Models\Thread::getContext()

  at vendor/laravel/framework/src/Illuminate/Support/Traits/ForwardsCalls.php:67
     63▕      * @throws \BadMethodCallException
     64▕      */
     65▕     protected static function throwBadMethodCallException($method)
     66▕     {
  ➜  67▕         throw new BadMethodCallException(sprintf(
     68▕             'Call to undefined method %s::%s()', static::class, $method
     69▕         ));
     70▕     }
     71▕ }

      +3 vendor frames
  4   tests/Unit/ProjectTest.php:143


  Tests:    2 failed, 1 skipped, 11 passed (20 assertions)
