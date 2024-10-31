  openagents git:(teams) pf CreateTeamT

   FAIL  Tests\Feature\CreateTeamTest
  ✓ authenticated user can create a new team                                                           0.11s
  ✓ team name is required                                                                              0.01s
  ⨯ team name must be unique for the creating user                                                     0.01s
  ✓ guest cannot create a team                                                                         0.01s
  ⨯ user can switch between teams                                                                      0.01s
  ✓ user can switch to personal context                                                                0.01s
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CreateTeamTest > team name must be unique for the creating user
  Session is missing expected key [errors].
Failed asserting that false is true.

  at tests/Feature/CreateTeamTest.php:49
     45▕         ->post('/teams', [
     46▕             'name' => 'Existing Team'
     47▕         ]);
     48▕
  ➜  49▕     $response->assertSessionHasErrors(['name']);
     50▕ });
     51▕
     52▕ test('guest cannot create a team', function () {
     53▕     $response = $this->post('/teams', [

  ──────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CreateTeamTest > user can switch between teams
  Failed asserting that 1 matches expected 2.

  at tests/Feature/CreateTeamTest.php:79
     75▕             'team_id' => $team2->id
     76▕         ]);
     77▕
     78▕     $response->assertRedirect();
  ➜  79▕     $this->assertEquals($team2->id, $user->fresh()->current_team_id);
     80▕ });
     81▕
     82▕ test('user can switch to personal context', function () {
     83▕     $user = User::factory()->create();

  1   tests/Feature/CreateTeamTest.php:79


  Tests:    2 failed, 4 passed (14 assertions)
  Duration: 0.20s
