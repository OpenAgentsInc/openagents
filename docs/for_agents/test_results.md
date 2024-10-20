  FAIL  Tests\Feature\LoadTeamsAndProjectsTest
  ⨯ HTMX endpoint returns teams and projects for active team                                0.09s
  ⨯ HTMX endpoint returns personal projects when no active team                             0.01s
  ⨯ switching teams updates the active team and projects                                    0.01s
  ───────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\LoadTeamsAndProjectsTest > HTMX endpoint returns teams and projects fo…
  Response does not contain Project 1
Failed asserting that false is true.

  at tests/Feature/LoadTeamsAndProjectsTest.php:44
     40▕     $responseContent = $response->getContent();
     41▕
     42▕     $this->assertTrue(str_contains($responseContent, 'Team 1'), 'Response does not contain Team 1');
     43▕     $this->assertTrue(str_contains($responseContent, 'Team 2'), 'Response does not contain Team 2');
  ➜  44▕     $this->assertTrue(str_contains($responseContent, 'Project 1'), 'Response does not contain Project 1');
     45▕     $this->assertTrue(str_contains($responseContent, 'Project 2'), 'Response does not contain Project 2');
     46▕     $this->assertFalse(str_contains($responseContent, 'Project 3'), 'Response contains Project 3 when it should not');
     47▕
     48▕     $response->assertSee('id="teamSwitcher"', false);

  1   tests/Feature/LoadTeamsAndProjectsTest.php:44

  ───────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\LoadTeamsAndProjectsTest > HTMX endpoint returns personal projects whe…
  Response does not contain Personal Project
Failed asserting that false is true.

  at tests/Feature/LoadTeamsAndProjectsTest.php:71
     67▕
     68▕     $responseContent = $response->getContent();
     69▕
     70▕     $this->assertTrue(str_contains($responseContent, 'Team 1'), 'Response does not contain Team 1');
  ➜  71▕     $this->assertTrue(str_contains($responseContent, 'Personal Project'), 'Response does not contain Personal Project');
     72▕     $this->assertFalse(str_contains($responseContent, 'Project 1'), 'Response contains Project 1 when it should not');
     73▕     $this->assertFalse(str_contains($responseContent, 'Project 2'), 'Response contains Project 2 when it should not');
     74▕
     75▕     // Log the personal projects

  1   tests/Feature/LoadTeamsAndProjectsTest.php:71

  ───────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\LoadTeamsAndProjectsTest > switching teams updates the active team and…
  Response does not contain Project 3
Failed asserting that false is true.

  at tests/Feature/LoadTeamsAndProjectsTest.php:91
     87▕
     88▕     $responseContent = $response->getContent();
     89▕
     90▕     $this->assertTrue(str_contains($responseContent, 'Team 2'), 'Response does not contain Team 2');
  ➜  91▕     $this->assertTrue(str_contains($responseContent, 'Project 3'), 'Response does not contain Project 3');
     92▕     $this->assertFalse(str_contains($responseContent, 'Project 1'), 'Response contains Project 1 when it should not');
     93▕     $this->assertFalse(str_contains($responseContent, 'Project 2'), 'Response contains Project 2 when it should not');
     94▕
     95▕     // Log the projects associated with the new active team

  1   tests/Feature/LoadTeamsAndProjectsTest.php:91


  Tests:    3 failed (10 assertions)
