
   PASS  Tests\Unit\MessageTest
  ✓ a message belongs to a user                                                                                                           0.08s
  ✓ a message belongs to a thread
  ✓ a message can be created by the system

   PASS  Tests\Unit\ProjectTest
  ✓ a project belongs to a user                                                                                                           0.01s
  ✓ a project belongs to a team
  ✓ a project has many threads
  ✓ a project belongs to either a user or a team

   PASS  Tests\Unit\TeamTest
  ✓ a team has many users
  ✓ a team has many projects
  ✓ a team has many threads through projects

   PASS  Tests\Unit\ThreadTest
  ✓ a thread belongs to a user                                                                                                            0.01s
  ✓ a thread belongs to a project
  ✓ a thread has many messages                                                                                                            0.01s

   PASS  Tests\Unit\UserTest
  ✓ a user belongs to a team
  ✓ a user has many projects
  ✓ a user has many threads
  ✓ a user has many messages                                                                                                              0.01s
  ✓ a user can have projects through their team

   PASS  Tests\Feature\Auth\AuthenticationTest
  ✓ users can authenticate using the login screen                                                                                         0.04s
  ✓ users can not authenticate with invalid password                                                                                      0.22s
  ✓ users can logout                                                                                                                      0.01s

   PASS  Tests\Feature\Auth\EmailVerificationTest
  ✓ email can be verified                                                                                                                 0.02s
  ✓ email is not verified with invalid hash                                                                                               0.01s

   PASS  Tests\Feature\Auth\PasswordResetTest
  ✓ reset password link can be requested                                                                                                  0.02s
  ✓ password can be reset with valid token                                                                                                0.01s

   PASS  Tests\Feature\Auth\RegistrationTest
  ✓ new users can register                                                                                                                0.01s

   PASS  Tests\Feature\ComponentLibraryTest
  ✓ component library loads component library view                                                                                        0.01s

   FAIL  Tests\Feature\CoreFunctionalityTest
  ⨯ user can send a message in a thread                                                                                                   0.01s
  ⨯ system can add a message to a thread
  ⨯ threads can be organized into projects                                                                                                0.01s
  ⨯ threads can be organized into teams                                                                                                   0.01s
  ⨯ system can make LLM tool calls with GitHub API                                                                                        0.01s

   PASS  Tests\Feature\HomepageTest
  ✓ homepage loads homepage view                                                                                                          0.01s
  ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > user can send a message in a thread
  Expected response status code [201] but received 404.
Failed asserting that 404 is identical to 201.

  at tests/Feature/CoreFunctionalityTest.php:20
     16▕         'thread_id' => $thread->id,
     17▕         'content' => 'Test message',
     18▕     ]);
     19▕
  ➜  20▕     $response->assertStatus(201);
     21▕     $this->assertDatabaseHas('messages', [
     22▕         'thread_id' => $thread->id,
     23▕         'user_id' => $user->id,
     24▕         'content' => 'Test message',

  ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > system can add a message to a thread
  Expected response status code [201] but received 404.
Failed asserting that 404 is identical to 201.

  at tests/Feature/CoreFunctionalityTest.php:36
     32▕         'thread_id' => $thread->id,
     33▕         'content' => 'System response',
     34▕     ]);
     35▕
  ➜  36▕     $response->assertStatus(201);
     37▕     $this->assertDatabaseHas('messages', [
     38▕         'thread_id' => $thread->id,
     39▕         'user_id' => null,
     40▕         'content' => 'System response',

  ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > threads can be organized into projects
  Expected response status code [200] but received 404.
Failed asserting that 404 is identical to 200.

  at tests/Feature/CoreFunctionalityTest.php:50
     46▕     $thread = Thread::factory()->create(['project_id' => $project->id]);
     47▕
     48▕     $response = $this->get("/projects/{$project->id}/threads");
     49▕
  ➜  50▕     $response->assertStatus(200);
     51▕     $response->assertJson([$thread->toArray()]);
     52▕ });
     53▕
     54▕ test('threads can be organized into teams', function () {

  ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > threads can be organized into teams                                            QueryException
  SQLSTATE[HY000]: General error: 1 table threads has no column named team_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "team_id", "updated_at", "created_at") values (Velit in perferendis ad ut similique qui., 1, 1, 2024-10-17 15:29:46, 2024-10-17 15:29:46))

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
  17  tests/Feature/CoreFunctionalityTest.php:56

  ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > system can make LLM tool calls with GitHub API
  Expected response status code [200] but received 404.
Failed asserting that 404 is identical to 200.

  at tests/Feature/CoreFunctionalityTest.php:90
     86▕     $response = $this->post("/threads/{$thread->id}/process", [
     87▕         'message_id' => $message->id,
     88▕     ]);
     89▕
  ➜  90▕     $response->assertStatus(200);
     91▕     $response->assertJson(['success' => true]);
     92▕ });
     93▕


  Tests:    5 failed, 28 passed (66 assertions)
