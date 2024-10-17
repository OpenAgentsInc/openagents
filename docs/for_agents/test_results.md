 FAIL  Tests\Unit\ThreadTest
  ✓ a thread belongs to a user                                                                                           0.01s
  ⨯ a thread belongs to a project
  ⨯ a thread has many messages
  ⨯ a thread belongs to a team through a project

   FAIL  Tests\Unit\UserTest
  ✓ a user belongs to a team                                                                                             0.01s
  ✓ a user has many projects
  ✓ a user has many threads
  ⨯ a user has many messages                                                                                             0.01s
  ✓ a user can have projects through their team

   FAIL  Tests\Feature\Auth\AuthenticationTest
  ⨯ login screen can be rendered                                                                                         0.27s
  ⨯ users can authenticate using the login screen                                                                        0.03s
  ✓ users can not authenticate with invalid password                                                                     0.22s
  ⨯ users can logout                                                                                                     0.01s

   PASS  Tests\Feature\Auth\EmailVerificationTest
  ✓ email can be verified                                                                                                0.02s
  ✓ email is not verified with invalid hash                                                                              0.01s

   PASS  Tests\Feature\Auth\PasswordResetTest
  ✓ reset password link can be requested                                                                                 0.02s
  ✓ password can be reset with valid token                                                                               0.01s

   FAIL  Tests\Feature\Auth\RegistrationTest
  ⨯ registration screen can be rendered                                                                                  0.22s
  ⨯ new users can register                                                                                               0.01s

   PASS  Tests\Feature\ComponentLibraryTest
  ✓ component library loads component library view                                                                       0.01s

   FAIL  Tests\Feature\CoreFunctionalityTest
  ⨯ user can send a message in a thread                                                                                  0.01s
  ⨯ system can add a message to a thread
  ⨯ threads can be organized into projects                                                                               0.01s
  ⨯ threads can be organized into teams                                                                                  0.01s
  ⨯ system can make LLM tool calls with GitHub API

   FAIL  Tests\Feature\HomepageTest
  ✓ homepage loads homepage view for unauthenticated users                                                               0.01s
  ⨯ homepage loads dashboard view for authenticated users                                                                0.34s

   PASS  Tests\Feature\SendMessageTest
  ✓ authenticated user can send a message                                                                                0.01s
  ✓ authenticated user can send a message to an existing thread                                                          0.01s
  ✓ unauthenticated user cannot send a message
  ✓ message cannot be empty                                                                                              0.01s
  ✓ project_id is required
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\MessageTest > a message belongs to a user                                                QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Accusantium fuga sit ut fugit et quisquam inventore., 1, 2024-10-17 16:17:27, 2024-10-17 16:17:27))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +26 vendor frames
  27  tests/Unit/MessageTest.php:9

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\MessageTest > a message belongs to a thread                                              QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Dolorem laborum quibusdam a vel placeat quia., 1, 2024-10-17 16:17:27, 2024-10-17 16:17:27))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +15 vendor frames
  16  tests/Unit/MessageTest.php:16

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\MessageTest > a message can be created by the system                                     QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Molestiae fugit dolor non., 1, 2024-10-17 16:17:27, 2024-10-17 16:17:27))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +15 vendor frames
  16  tests/Unit/MessageTest.php:24

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\ProjectTest > a project has many threads                                                 QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Optio optio quidem qui et deserunt quia., 1, 2024-10-17 16:17:27, 2024-10-17 16:17:27))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +16 vendor frames
  17  tests/Unit/ProjectTest.php:24

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\TeamTest > a team has many threads through projects                                      QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Reprehenderit aut sint aliquam exercitationem distinctio sit., 1, 2024-10-17 16:17:27, 2024-10-17 16:17:27))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +16 vendor frames
  17  tests/Unit/TeamTest.php:27

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\ThreadTest > a thread belongs to a project                                               QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Optio saepe nisi blanditiis sunt officiis quae culpa., 1, 2024-10-17 16:17:27, 2024-10-17 16:17:27))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +16 vendor frames
  17  tests/Unit/ThreadTest.php:19

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\ThreadTest > a thread has many messages                                                  QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Porro tempora esse dolorem asperiores., 1, 2024-10-17 16:17:27, 2024-10-17 16:17:27))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +15 vendor frames
  16  tests/Unit/ThreadTest.php:26

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\ThreadTest > a thread belongs to a team through a project                                QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Temporibus iusto animi maiores veritatis exercitationem consequatur excepturi., 1, 2024-10-17 16:17:27, 2024-10-17 16:17:27))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +16 vendor frames
  17  tests/Unit/ThreadTest.php:36

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\UserTest > a user has many messages                                                      QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Omnis odio quas quaerat voluptas qui est omnis., 1, 2024-10-17 16:17:27, 2024-10-17 16:17:27))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +27 vendor frames
  28  tests/Unit/UserTest.php:35

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\Auth\AuthenticationTest > login screen can be rendered
  Expected response status code [200] but received 405.
Failed asserting that 405 is identical to 200.

  at tests/Feature/Auth/AuthenticationTest.php:8
      4▕
      5▕ test('login screen can be rendered', function () {
      6▕     $response = $this->get('/login');
      7▕
  ➜   8▕     $response->assertStatus(200);
      9▕ });
     10▕
     11▕ test('users can authenticate using the login screen', function () {
     12▕     $user = User::factory()->create();

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\Auth\AuthenticationTest > users can authenticate using the login screen
  Expected response status code [201, 301, 302, 303, 307, 308] but received 204.
Failed asserting that false is true.

  at tests/Feature/Auth/AuthenticationTest.php:20
     16▕         'password' => 'password',
     17▕     ]);
     18▕
     19▕     $this->assertAuthenticated();
  ➜  20▕     $response->assertRedirect('/');
     21▕ });
     22▕
     23▕ test('users can not authenticate with invalid password', function () {
     24▕     $user = User::factory()->create();

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\Auth\AuthenticationTest > users can logout
  Expected response status code [201, 301, 302, 303, 307, 308] but received 204.
Failed asserting that false is true.

  at tests/Feature/Auth/AuthenticationTest.php:40
     36▕
     37▕     $response = $this->actingAs($user)->post('/logout');
     38▕
     39▕     $this->assertGuest();
  ➜  40▕     $response->assertRedirect('/');
     41▕ });

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\Auth\RegistrationTest > registration screen can be rendered
  Expected response status code [200] but received 405.
Failed asserting that 405 is identical to 200.

  at tests/Feature/Auth/RegistrationTest.php:8
      4▕
      5▕ test('registration screen can be rendered', function () {
      6▕     $response = $this->get('/register');
      7▕
  ➜   8▕     $response->assertStatus(200);
      9▕ });
     10▕
     11▕ test('new users can register', function () {
     12▕     $response = $this->post('/register', [

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\Auth\RegistrationTest > new users can register
  Expected response status code [201, 301, 302, 303, 307, 308] but received 204.
Failed asserting that false is true.

  at tests/Feature/Auth/RegistrationTest.php:20
     16▕         'password_confirmation' => 'password',
     17▕     ]);
     18▕
     19▕     $this->assertAuthenticated();
  ➜  20▕     $response->assertRedirect('/');
     21▕ });

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > user can send a message in a thread                           QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Ipsum fugiat ab autem aut voluptatem et., 1, 2024-10-17 16:17:28, 2024-10-17 16:17:28))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +15 vendor frames
  16  tests/Feature/CoreFunctionalityTest.php:13

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > system can add a message to a thread                          QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Voluptatem consequatur dolorum voluptate ipsam., 1, 2024-10-17 16:17:28, 2024-10-17 16:17:28))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +15 vendor frames
  16  tests/Feature/CoreFunctionalityTest.php:30

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > threads can be organized into projects                        QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Dicta dignissimos tenetur autem provident ex., 1, 2024-10-17 16:17:28, 2024-10-17 16:17:28))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +16 vendor frames
  17  tests/Feature/CoreFunctionalityTest.php:48

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > threads can be organized into teams                           QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Nam officiis odio molestiae velit., 1, 2024-10-17 16:17:28, 2024-10-17 16:17:28))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +16 vendor frames
  17  tests/Feature/CoreFunctionalityTest.php:60

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > system can make LLM tool calls with GitHub API                QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: threads.user_id (Connection: sqlite, SQL: insert into "threads" ("title", "project_id", "updated_at", "created_at") values (Et eos voluptas maxime consectetur recusandae., 1, 2024-10-17 16:17:28, 2024-10-17 16:17:28))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:571
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();
    570▕
  ➜ 571▕             return $statement->execute();
    572▕         });
    573▕     }
    574▕
    575▕     /**

      +15 vendor frames
  16  tests/Feature/CoreFunctionalityTest.php:70

  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\HomepageTest > homepage loads dashboard view for authenticated users
  Expected response status code [200] but received 500.
Failed asserting that 500 is identical to 200.

The following exception occurred during the last request:

ErrorException: Attempt to read property "id" on null in /Users/christopherdavid/code/openagents/storage/framework/views/3deb603fb8c187e9b6a4cae29bd783a6.php:47
Stack trace:
#0 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Bootstrap/HandleExceptions.php(256): Illuminate\Foundation\Bootstrap\HandleExceptions->handleError(2, 'Attempt to read...', '/Users/christop...', 47)
#1 /Users/christopherdavid/code/openagents/storage/framework/views/3deb603fb8c187e9b6a4cae29bd783a6.php(47): Illuminate\Foundation\Bootstrap\HandleExceptions->Illuminate\Foundation\Bootstrap\{closure}(2, 'Attempt to read...', '/Users/christop...', 47)
#2 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Filesystem/Filesystem.php(123): require('/Users/christop...')
#3 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Filesystem/Filesystem.php(124): Illuminate\Filesystem\Filesystem::Illuminate\Filesystem\{closure}()
#4 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/Engines/PhpEngine.php(58): Illuminate\Filesystem\Filesystem->getRequire('/Users/christop...', Array)
#5 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/Engines/CompilerEngine.php(74): Illuminate\View\Engines\PhpEngine->evaluatePath('/Users/christop...', Array)
#6 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/View.php(208): Illuminate\View\Engines\CompilerEngine->get('/Users/christop...', Array)
#7 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/View.php(191): Illuminate\View\View->getContents()
#8 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/View.php(160): Illuminate\View\View->renderContents()
#9 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Response.php(70): Illuminate\View\View->render()
#10 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Response.php(35): Illuminate\Http\Response->setContent(Object(Illuminate\View\View))
#11 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(920): Illuminate\Http\Response->__construct(Object(Illuminate\View\View), 200, Array)
#12 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(887): Illuminate\Routing\Router::toResponse(Object(Illuminate\Http\Request), Object(Illuminate\View\View))
#13 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(807): Illuminate\Routing\Router->prepareResponse(Object(Illuminate\Http\Request), Object(Illuminate\View\View))
#14 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(144): Illuminate\Routing\Router->Illuminate\Routing\{closure}(Object(Illuminate\Http\Request))
#15 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Middleware/SubstituteBindings.php(51): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#16 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Routing\Middleware\SubstituteBindings->handle(Object(Illuminate\Http\Request), Object(Closure))
#17 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/VerifyCsrfToken.php(88): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#18 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\VerifyCsrfToken->handle(Object(Illuminate\Http\Request), Object(Closure))
#19 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/Middleware/ShareErrorsFromSession.php(49): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#20 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\View\Middleware\ShareErrorsFromSession->handle(Object(Illuminate\Http\Request), Object(Closure))
#21 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Session/Middleware/StartSession.php(121): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#22 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Session/Middleware/StartSession.php(64): Illuminate\Session\Middleware\StartSession->handleStatefulRequest(Object(Illuminate\Http\Request), Object(Illuminate\Session\Store), Object(Closure))
#23 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Session\Middleware\StartSession->handle(Object(Illuminate\Http\Request), Object(Closure))
#24 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Cookie/Middleware/AddQueuedCookiesToResponse.php(37): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#25 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Cookie\Middleware\AddQueuedCookiesToResponse->handle(Object(Illuminate\Http\Request), Object(Closure))
#26 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Cookie/Middleware/EncryptCookies.php(75): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#27 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Cookie\Middleware\EncryptCookies->handle(Object(Illuminate\Http\Request), Object(Closure))
#28 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(119): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#29 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(807): Illuminate\Pipeline\Pipeline->then(Object(Closure))
#30 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(786): Illuminate\Routing\Router->runRouteWithinStack(Object(Illuminate\Routing\Route), Object(Illuminate\Http\Request))
#31 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(750): Illuminate\Routing\Router->runRoute(Object(Illuminate\Http\Request), Object(Illuminate\Routing\Route))
#32 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(739): Illuminate\Routing\Router->dispatchToRoute(Object(Illuminate\Http\Request))
#33 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Kernel.php(201): Illuminate\Routing\Router->dispatch(Object(Illuminate\Http\Request))
#34 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(144): Illuminate\Foundation\Http\Kernel->Illuminate\Foundation\Http\{closure}(Object(Illuminate\Http\Request))
#35 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/TransformsRequest.php(21): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#36 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/ConvertEmptyStringsToNull.php(31): Illuminate\Foundation\Http\Middleware\TransformsRequest->handle(Object(Illuminate\Http\Request), Object(Closure))
#37 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\ConvertEmptyStringsToNull->handle(Object(Illuminate\Http\Request), Object(Closure))
#38 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/TransformsRequest.php(21): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#39 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/TrimStrings.php(51): Illuminate\Foundation\Http\Middleware\TransformsRequest->handle(Object(Illuminate\Http\Request), Object(Closure))
#40 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\TrimStrings->handle(Object(Illuminate\Http\Request), Object(Closure))
#41 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Middleware/ValidatePostSize.php(27): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#42 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Http\Middleware\ValidatePostSize->handle(Object(Illuminate\Http\Request), Object(Closure))
#43 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/PreventRequestsDuringMaintenance.php(110): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#44 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\PreventRequestsDuringMaintenance->handle(Object(Illuminate\Http\Request), Object(Closure))
#45 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Middleware/HandleCors.php(62): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#46 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Http\Middleware\HandleCors->handle(Object(Illuminate\Http\Request), Object(Closure))
#47 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Middleware/TrustProxies.php(58): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#48 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Http\Middleware\TrustProxies->handle(Object(Illuminate\Http\Request), Object(Closure))
#49 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/InvokeDeferredCallbacks.php(22): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#50 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\InvokeDeferredCallbacks->handle(Object(Illuminate\Http\Request), Object(Closure))
#51 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(119): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#52 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Kernel.php(176): Illuminate\Pipeline\Pipeline->then(Object(Closure))
#53 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Kernel.php(145): Illuminate\Foundation\Http\Kernel->sendRequestThroughRouter(Object(Illuminate\Http\Request))
#54 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Testing/Concerns/MakesHttpRequests.php(604): Illuminate\Foundation\Http\Kernel->handle(Object(Illuminate\Http\Request))
#55 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Testing/Concerns/MakesHttpRequests.php(365): Illuminate\Foundation\Testing\TestCase->call('GET', '/', Array, Array, Array, Array)
#56 /Users/christopherdavid/code/openagents/tests/Feature/HomepageTest.php(15): Illuminate\Foundation\Testing\TestCase->get('/')
#57 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Factories/TestCaseMethodFactory.php(166): P\Tests\Feature\HomepageTest->{closure}()
#58 [internal function]: P\Tests\Feature\HomepageTest->Pest\Factories\{closure}()
#59 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Concerns/Testable.php(417): call_user_func_array(Object(Closure), Array)
#60 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Support/ExceptionTrace.php(26): P\Tests\Feature\HomepageTest->Pest\Concerns\{closure}()
#61 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Concerns/Testable.php(417): Pest\Support\ExceptionTrace::ensure(Object(Closure))
#62 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Concerns/Testable.php(319): P\Tests\Feature\HomepageTest->__callClosure(Object(Closure), Array)
#63 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Factories/TestCaseFactory.php(169) : eval()'d code(26): P\Tests\Feature\HomepageTest->__runTest(Object(Closure))
#64 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestCase.php(1234): P\Tests\Feature\HomepageTest->__pest_evaluable_homepage_loads_dashboard_view_for_authenticated_users()
#65 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestCase.php(515): PHPUnit\Framework\TestCase->runTest()
#66 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestRunner/TestRunner.php(86): PHPUnit\Framework\TestCase->runBare()
#67 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestCase.php(362): PHPUnit\Framework\TestRunner->run(Object(P\Tests\Feature\HomepageTest))
#68 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestSuite.php(375): PHPUnit\Framework\TestCase->run()
#69 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestSuite.php(375): PHPUnit\Framework\TestSuite->run()
#70 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestSuite.php(375): PHPUnit\Framework\TestSuite->run()
#71 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/TextUI/TestRunner.php(64): PHPUnit\Framework\TestSuite->run()
#72 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/TextUI/Application.php(209): PHPUnit\TextUI\TestRunner->run(Object(PHPUnit\TextUI\Configuration\Configuration), Object(PHPUnit\Runner\ResultCache\DefaultResultCache), Object(PHPUnit\Framework\TestSuite))
#73 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Kernel.php(103): PHPUnit\TextUI\Application->run(Array)
#74 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/bin/pest(184): Pest\Kernel->handle(Array, Array)
#75 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/bin/pest(192): {closure}()
#76 /Users/christopherdavid/code/openagents/vendor/bin/pest(119): include('/Users/christop...')
#77 {main}

Next Illuminate\View\ViewException: Attempt to read property "id" on null (View: /Users/christopherdavid/code/openagents/resources/views/dashboard.blade.php) in /Users/christopherdavid/code/openagents/storage/framework/views/3deb603fb8c187e9b6a4cae29bd783a6.php:47
Stack trace:
#0 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/Engines/PhpEngine.php(60): Illuminate\View\Engines\CompilerEngine->handleViewException(Object(ErrorException), 2)
#1 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/Engines/CompilerEngine.php(74): Illuminate\View\Engines\PhpEngine->evaluatePath('/Users/christop...', Array)
#2 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/View.php(208): Illuminate\View\Engines\CompilerEngine->get('/Users/christop...', Array)
#3 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/View.php(191): Illuminate\View\View->getContents()
#4 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/View.php(160): Illuminate\View\View->renderContents()
#5 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Response.php(70): Illuminate\View\View->render()
#6 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Response.php(35): Illuminate\Http\Response->setContent(Object(Illuminate\View\View))
#7 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(920): Illuminate\Http\Response->__construct(Object(Illuminate\View\View), 200, Array)
#8 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(887): Illuminate\Routing\Router::toResponse(Object(Illuminate\Http\Request), Object(Illuminate\View\View))
#9 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(807): Illuminate\Routing\Router->prepareResponse(Object(Illuminate\Http\Request), Object(Illuminate\View\View))
#10 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(144): Illuminate\Routing\Router->Illuminate\Routing\{closure}(Object(Illuminate\Http\Request))
#11 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Middleware/SubstituteBindings.php(51): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#12 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Routing\Middleware\SubstituteBindings->handle(Object(Illuminate\Http\Request), Object(Closure))
#13 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/VerifyCsrfToken.php(88): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#14 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\VerifyCsrfToken->handle(Object(Illuminate\Http\Request), Object(Closure))
#15 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/Middleware/ShareErrorsFromSession.php(49): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#16 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\View\Middleware\ShareErrorsFromSession->handle(Object(Illuminate\Http\Request), Object(Closure))
#17 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Session/Middleware/StartSession.php(121): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#18 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Session/Middleware/StartSession.php(64): Illuminate\Session\Middleware\StartSession->handleStatefulRequest(Object(Illuminate\Http\Request), Object(Illuminate\Session\Store), Object(Closure))
#19 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Session\Middleware\StartSession->handle(Object(Illuminate\Http\Request), Object(Closure))
#20 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Cookie/Middleware/AddQueuedCookiesToResponse.php(37): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#21 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Cookie\Middleware\AddQueuedCookiesToResponse->handle(Object(Illuminate\Http\Request), Object(Closure))
#22 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Cookie/Middleware/EncryptCookies.php(75): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#23 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Cookie\Middleware\EncryptCookies->handle(Object(Illuminate\Http\Request), Object(Closure))
#24 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(119): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#25 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(807): Illuminate\Pipeline\Pipeline->then(Object(Closure))
#26 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(786): Illuminate\Routing\Router->runRouteWithinStack(Object(Illuminate\Routing\Route), Object(Illuminate\Http\Request))
#27 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(750): Illuminate\Routing\Router->runRoute(Object(Illuminate\Http\Request), Object(Illuminate\Routing\Route))
#28 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Routing/Router.php(739): Illuminate\Routing\Router->dispatchToRoute(Object(Illuminate\Http\Request))
#29 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Kernel.php(201): Illuminate\Routing\Router->dispatch(Object(Illuminate\Http\Request))
#30 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(144): Illuminate\Foundation\Http\Kernel->Illuminate\Foundation\Http\{closure}(Object(Illuminate\Http\Request))
#31 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/TransformsRequest.php(21): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#32 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/ConvertEmptyStringsToNull.php(31): Illuminate\Foundation\Http\Middleware\TransformsRequest->handle(Object(Illuminate\Http\Request), Object(Closure))
#33 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\ConvertEmptyStringsToNull->handle(Object(Illuminate\Http\Request), Object(Closure))
#34 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/TransformsRequest.php(21): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#35 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/TrimStrings.php(51): Illuminate\Foundation\Http\Middleware\TransformsRequest->handle(Object(Illuminate\Http\Request), Object(Closure))
#36 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\TrimStrings->handle(Object(Illuminate\Http\Request), Object(Closure))
#37 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Middleware/ValidatePostSize.php(27): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#38 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Http\Middleware\ValidatePostSize->handle(Object(Illuminate\Http\Request), Object(Closure))
#39 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/PreventRequestsDuringMaintenance.php(110): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#40 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\PreventRequestsDuringMaintenance->handle(Object(Illuminate\Http\Request), Object(Closure))
#41 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Middleware/HandleCors.php(62): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#42 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Http\Middleware\HandleCors->handle(Object(Illuminate\Http\Request), Object(Closure))
#43 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Http/Middleware/TrustProxies.php(58): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#44 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Http\Middleware\TrustProxies->handle(Object(Illuminate\Http\Request), Object(Closure))
#45 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Middleware/InvokeDeferredCallbacks.php(22): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#46 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(183): Illuminate\Foundation\Http\Middleware\InvokeDeferredCallbacks->handle(Object(Illuminate\Http\Request), Object(Closure))
#47 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Pipeline/Pipeline.php(119): Illuminate\Pipeline\Pipeline->Illuminate\Pipeline\{closure}(Object(Illuminate\Http\Request))
#48 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Kernel.php(176): Illuminate\Pipeline\Pipeline->then(Object(Closure))
#49 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Http/Kernel.php(145): Illuminate\Foundation\Http\Kernel->sendRequestThroughRouter(Object(Illuminate\Http\Request))
#50 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Testing/Concerns/MakesHttpRequests.php(604): Illuminate\Foundation\Http\Kernel->handle(Object(Illuminate\Http\Request))
#51 /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/Foundation/Testing/Concerns/MakesHttpRequests.php(365): Illuminate\Foundation\Testing\TestCase->call('GET', '/', Array, Array, Array, Array)
#52 /Users/christopherdavid/code/openagents/tests/Feature/HomepageTest.php(15): Illuminate\Foundation\Testing\TestCase->get('/')
#53 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Factories/TestCaseMethodFactory.php(166): P\Tests\Feature\HomepageTest->{closure}()
#54 [internal function]: P\Tests\Feature\HomepageTest->Pest\Factories\{closure}()
#55 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Concerns/Testable.php(417): call_user_func_array(Object(Closure), Array)
#56 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Support/ExceptionTrace.php(26): P\Tests\Feature\HomepageTest->Pest\Concerns\{closure}()
#57 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Concerns/Testable.php(417): Pest\Support\ExceptionTrace::ensure(Object(Closure))
#58 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Concerns/Testable.php(319): P\Tests\Feature\HomepageTest->__callClosure(Object(Closure), Array)
#59 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Factories/TestCaseFactory.php(169) : eval()'d code(26): P\Tests\Feature\HomepageTest->__runTest(Object(Closure))
#60 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestCase.php(1234): P\Tests\Feature\HomepageTest->__pest_evaluable_homepage_loads_dashboard_view_for_authenticated_users()
#61 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestCase.php(515): PHPUnit\Framework\TestCase->runTest()
#62 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestRunner/TestRunner.php(86): PHPUnit\Framework\TestCase->runBare()
#63 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestCase.php(362): PHPUnit\Framework\TestRunner->run(Object(P\Tests\Feature\HomepageTest))
#64 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestSuite.php(375): PHPUnit\Framework\TestCase->run()
#65 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestSuite.php(375): PHPUnit\Framework\TestSuite->run()
#66 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/Framework/TestSuite.php(375): PHPUnit\Framework\TestSuite->run()
#67 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/TextUI/TestRunner.php(64): PHPUnit\Framework\TestSuite->run()
#68 /Users/christopherdavid/code/openagents/vendor/phpunit/phpunit/src/TextUI/Application.php(209): PHPUnit\TextUI\TestRunner->run(Object(PHPUnit\TextUI\Configuration\Configuration), Object(PHPUnit\Runner\ResultCache\DefaultResultCache), Object(PHPUnit\Framework\TestSuite))
#69 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/src/Kernel.php(103): PHPUnit\TextUI\Application->run(Array)
#70 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/bin/pest(184): Pest\Kernel->handle(Array, Array)
#71 /Users/christopherdavid/code/openagents/vendor/pestphp/pest/bin/pest(192): {closure}()
#72 /Users/christopherdavid/code/openagents/vendor/bin/pest(119): include('/Users/christop...')
#73 {main}

----------------------------------------------------------------------------------

Attempt to read property "id" on null (View: /Users/christopherdavid/code/openagents/resources/views/dashboard.blade.php)

  at tests/Feature/HomepageTest.php:17
     13▕     $user = User::factory()->create();
     14▕
     15▕     $response = $this->actingAs($user)->get('/');
     16▕
  ➜  17▕     $response->assertStatus(200);
     18▕     $response->assertViewIs('dashboard');
     19▕ });


  Tests:    20 failed, 22 passed (61 assertions)
