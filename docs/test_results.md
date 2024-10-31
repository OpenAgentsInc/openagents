
   FAIL  Tests\Feature\CreateTeamTest
  ⨯ authenticated user can create a new team                                                           0.29s
  ⨯ team name is required                                                                              0.07s
  ⨯ team name must be unique for the creating user                                                     0.01s
  ⨯ guest cannot create a team                                                                         0.06s
  ⨯ newly created team becomes users current team                                                      0.07s
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CreateTeamTest > authenticated user can create a new team
  Expected response status code [201, 301, 302, 303, 307, 308] but received 405.
Failed asserting that false is true.

  at tests/Feature/CreateTeamTest.php:15
     11▕         ->post('/teams', [
     12▕             'name' => 'Test Team'
     13▕         ]);
     14▕
  ➜  15▕     $response->assertRedirect();
     16▕
     17▕     $this->assertDatabaseHas('teams', [
     18▕         'name' => 'Test Team'
     19▕     ]);

  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CreateTeamTest > team name is required
  Session is missing expected key [errors].
Failed asserting that false is true.

  at tests/Feature/CreateTeamTest.php:35
     31▕         ->post('/teams', [
     32▕             'name' => ''
     33▕         ]);
     34▕
  ➜  35▕     $response->assertSessionHasErrors(['name']);
     36▕ });
     37▕
     38▕ test('team name must be unique for the creating user', function () {
     39▕     $user = User::factory()->create();

  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CreateTeamTest > team name must be unique for the creating user     QueryException
  SQLSTATE[HY000]: General error: 1 table teams has no column named name (Connection: sqlite, SQL: insert into "teams" ("name", "updated_at", "created_at") values (Existing Team, 2024-10-31 15:23:53, 2024-10-31 15:23:53))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:565
    561▕             if ($this->pretending()) {
    562▕                 return true;
    563▕             }
    564▕
  ➜ 565▕             $statement = $this->getPdo()->prepare($query);
    566▕
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();

      +16 vendor frames
  17  tests/Feature/CreateTeamTest.php:40

  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CreateTeamTest > guest cannot create a team
  Expected response status code [201, 301, 302, 303, 307, 308] but received 405.
Failed asserting that false is true.

  at tests/Feature/CreateTeamTest.php:57
     53▕     $response = $this->post('/teams', [
     54▕         'name' => 'Test Team'
     55▕     ]);
     56▕
  ➜  57▕     $response->assertRedirect('/login');
     58▕     $this->assertDatabaseMissing('teams', [
     59▕         'name' => 'Test Team'
     60▕     ]);
     61▕ });

  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CreateTeamTest > newly created team becomes users current team      ErrorException
  Attempt to read property "id" on null

  at tests/Feature/CreateTeamTest.php:77
     73▕             'name' => 'New Team'
     74▕         ]);
     75▕
     76▕     $newTeam = Team::where('name', 'New Team')->first();
  ➜  77▕     $this->assertEquals($newTeam->id, $user->fresh()->current_team_id);
     78▕ });


  Tests:    5 failed (3 assertions)
  Duration: 0.59s
