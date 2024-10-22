  PASS  Tests\Unit\MessageTest
  ✓ a message belongs to a user                                                                 0.08s
  ✓ a message belongs to a thread                                                               0.01s
  ✓ a message can be created by the system

   PASS  Tests\Unit\ProjectTest
  ✓ a project belongs to a user                                                                 0.01s
  ✓ a project belongs to a team
  ✓ a project has many threads                                                                  0.01s
  ✓ a project belongs to either a user or a team                                                0.01s

   PASS  Tests\Unit\TeamTest
  ✓ a team has many users                                                                       0.01s
  ✓ a team has many projects
  ✓ a team has many threads through projects

   PASS  Tests\Unit\ThreadTest
  ✓ a thread belongs to a user                                                                  0.01s
  ✓ a thread belongs to a project
  ✓ a thread has many messages                                                                  0.01s
  ✓ a thread belongs to a team through a project

   PASS  Tests\Unit\UserTest
  ✓ a user belongs to many teams                                                                0.01s
  ✓ a user has many projects
  ✓ a user has many threads
  ✓ a user has many messages                                                                    0.01s
  ✓ a user can have projects through their teams
  ✓ a user can create thread, respecting team/project                                           0.01s

   PASS  Tests\Feature\Auth\AuthenticationTest
  ✓ login screen can be rendered                                                                0.02s
  ✓ users can authenticate using the login screen                                               0.01s
  ✓ users can not authenticate with invalid password                                            0.21s
  ✓ users can logout                                                                            0.01s

   PASS  Tests\Feature\Auth\EmailVerificationTest
  ✓ email verification screen can be rendered                                                   0.01s
  ✓ email can be verified                                                                       0.01s
  ✓ email is not verified with invalid hash                                                     0.01s

   PASS  Tests\Feature\Auth\PasswordConfirmationTest
  ✓ confirm password screen can be rendered                                                     0.01s
  ✓ password can be confirmed                                                                   0.01s
  ✓ password is not confirmed with invalid password                                             0.21s

   PASS  Tests\Feature\Auth\PasswordResetTest
  ✓ reset password link screen can be rendered                                                  0.01s
  ✓ reset password link can be requested                                                        0.01s
  ✓ reset password screen can be rendered                                                       0.01s
  ✓ password can be reset with valid token                                                      0.01s

   PASS  Tests\Feature\Auth\PasswordUpdateTest
  ✓ password can be updated                                                                     0.01s
  ✓ correct password must be provided to update password                                        0.01s

   PASS  Tests\Feature\Auth\RegistrationTest
  ✓ registration screen can be rendered                                                         0.01s
  ✓ new users can register                                                                      0.01s

   PASS  Tests\Feature\ChatControllerTest
  ✓ unauthenticated user cannot fetch threads                                                   0.01s
  ✓ authenticated user can fetch threads for a team                                             0.01s

   PASS  Tests\Feature\ComponentLibraryTest
  ✓ component library loads component library view                                              0.01s

   FAIL  Tests\Feature\CoreFunctionalityTest
  ✓ user can send a message in a thread                                                         0.01s
  ⨯ system can add a message to a thread                                                        0.01s
  ⨯ threads can be organized into projects                                                      0.01s
  ⨯ threads can be organized into teams                                                         0.01s
  ⨯ system can make LLM tool calls with GitHub API                                              0.01s

   FAIL  Tests\Feature\HTMXChatViewTest
  ⨯ clicking a chat updates main content with correct HTML                                      0.01s
  ⨯ sending a message updates the chat content                                                  0.01s

   FAIL  Tests\Feature\HTMXTest
  ⨯ creating a new thread updates sidebar and main content                                      0.30s
  ⨯ selecting a thread updates main content without full page reload                            0.01s
  ⨯ switching projects updates thread list in sidebar                                           0.01s

   FAIL  Tests\Feature\HomepageChatTest
  ✓ authenticated user can send message from homepage and is redirected to new chat thread      0.01s
  ✓ unauthenticated user is redirected to login when trying to send message from homepage
  ⨯ chat page loads correctly after sending message                                             0.01s

   PASS  Tests\Feature\HomepageTest
  ✓ homepage loads dashboard view for unauthenticated users                                     0.01s
  ✓ homepage loads dashboard view for authenticated users                                       0.01s

   FAIL  Tests\Feature\LoadTeamsAndProjectsTest
  ⨯ initial page load does not contain teams and projects                                       0.01s
  ✓ HTMX endpoint returns teams and projects for active team                                    0.01s
  ✓ HTMX endpoint does not return teams and projects not associated with the user               0.01s
  ✓ HTMX endpoint returns teams and personal projects when no active team                       0.01s
  ✓ switching teams updates the active team and projects                                        0.01s
  ✓ switching projects updates the active project                                               0.01s

   PASS  Tests\Feature\SendMessageTest
  ✓ authenticated user can send a message without a project                                     0.01s
  ✓ authenticated user can send a message with a project                                        0.01s
  ✓ authenticated user can send a message to an existing thread                                 0.01s
  ✓ unauthenticated user cannot send a message
  ✓ message cannot be empty                                                                     0.01s
  ✓ project_id must be valid if provided

   PASS  Tests\Feature\TeamAndProjectSwitchTest
  ✓ user can switch team                                                                        0.01s
  ✓ user can switch project                                                                     0.01s
  ✓ user cannot switch to a team they do not belong to                                          0.01s
  ✓ user cannot switch to a project they do not have access to                                  0.01s
  ✓ switching teams resets current project                                                      0.01s
  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > system can add a message to a thread
  Expected response status code [201] but received 404.
Failed asserting that 404 is identical to 201.

  at tests/Feature/CoreFunctionalityTest.php:38
     34▕         'content' => 'System response',
     35▕         'user_id' => null,
     36▕     ]);
     37▕
  ➜  38▕     $response->assertStatus(201);
     39▕     $this->assertDatabaseHas('messages', [
     40▕         'thread_id' => $thread->id,
     41▕         'user_id' => null,
     42▕         'content' => 'System response',

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > threads can be organized into projects
  Expected response status code [200] but received 404.
Failed asserting that 404 is identical to 200.

  at tests/Feature/CoreFunctionalityTest.php:54
     50▕     $thread = Thread::factory()->create(['project_id' => $project->id]);
     51▕
     52▕     $response = $this->actingAs($user)->get("/projects/{$project->id}/threads");
     53▕
  ➜  54▕     $response->assertStatus(200);
     55▕     $response->assertJsonFragment($thread->toArray());
     56▕ });
     57▕
     58▕ test('threads can be organized into teams', function () {

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > threads can be organized into teams
  Expected response status code [200] but received 404.
Failed asserting that 404 is identical to 200.

  at tests/Feature/CoreFunctionalityTest.php:66
     62▕     $thread = Thread::factory()->create(['project_id' => $project->id]);
     63▕
     64▕     $response = $this->actingAs($user)->get("/teams/{$team->id}/threads");
     65▕
  ➜  66▕     $response->assertStatus(200);
     67▕     $response->assertJsonFragment($thread->toArray());
     68▕ });
     69▕
     70▕ test('system can make LLM tool calls with GitHub API', function () {

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\CoreFunctionalityTest > system can make LLM tool calls with GitHub API
  Expected response status code [200] but received 404.
Failed asserting that 404 is identical to 200.

  at tests/Feature/CoreFunctionalityTest.php:97
     93▕     $response = $this->actingAs($user)->post("/threads/{$thread->id}/process", [
     94▕         'message_id' => $message->id,
     95▕     ]);
     96▕
  ➜  97▕     $response->assertStatus(200);
     98▕     $response->assertJson(['success' => true]);
     99▕ });

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\HTMXChatViewTest > clicking a chat updates main content with correct HTML
  Expected: <!DOCTYPE html>\n
  <html lang="en" class="dark">\n
  \n
  ... (339 more lines)

  To contain: id="main-content-inner"

  at tests/Feature/HTMXChatViewTest.php:24
     20▕     // Assert the response status is 200 (OK)
     21▕     $response->assertStatus(200);
     22▕
     23▕     // Assert that the response contains the expected HTML structure
  ➜  24▕     $response->assertSee('id="main-content-inner"', false);
     25▕     $response->assertSee('id="chat-content"', false);
     26▕     $response->assertSee('id="message-list"', false);
     27▕
     28▕     // Assert that the response contains the thread title

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\HTMXChatViewTest > sending a message updates the chat content
  Expected response status code [200] but received 302.
Failed asserting that 302 is identical to 200.

  at tests/Feature/HTMXChatViewTest.php:59
     55▕             'content' => 'Test message'
     56▕         ]);
     57▕
     58▕     // Assert the response status is 200 (OK)
  ➜  59▕     $response->assertStatus(200);
     60▕
     61▕     // Assert that the response contains the new message
     62▕     $response->assertSee('Test message');
     63▕

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\HTMXTest > creating a new thread updates sidebar and main content
  Expected response status code [200] but received 500.
Failed asserting that 500 is identical to 200.

The following exception occurred during the last request:

InvalidArgumentException: View [chat.messages] not found. in /Users/christopherdavid/code/openagents/vendor/laravel/framework/src/Illuminate/View/FileViewFinder.php:139
Stack trace:
...
----------------------------------------------------------------------------------

View [chat.messages] not found.

  at tests/Feature/HTMXTest.php:23
     19▕     $response = $this->actingAs($user)
     20▕         ->withHeaders(['HX-Request' => 'true'])
     21▕         ->post(route('threads.create'));
     22▕
  ➜  23▕     $response->assertStatus(200);
     24▕     $response->assertJsonStructure([
     25▕         'threadList',
     26▕         'chatContent',
     27▕         'url'

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\HTMXTest > selecting a thread updates main content without full page reloa…
  Failed asserting that two strings are equal.
  -'chat.show'
  +'components.chat.index'


  at tests/Feature/HTMXTest.php:56
     52▕         ->withHeaders(['HX-Request' => 'true'])
     53▕         ->get(route('chat.show', $thread->id));
     54▕
     55▕     $response->assertStatus(200);
  ➜  56▕     $response->assertViewIs('chat.show');
     57▕     $response->assertViewHas('thread', $thread);
     58▕     $response->assertViewHas('messages');
     59▕
     60▕     // Check that only the main content is returned, not a full page

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\HTMXTest > switching projects updates thread list in sidebar
  Failed asserting that two strings are equal.
  -'partials.thread-list'
  +'components.sidebar.thread-list'


  at tests/Feature/HTMXTest.php:81
     77▕         ->withHeaders(['HX-Request' => 'true'])
     78▕         ->get(route('threads.index', ['project_id' => $project2->id]));
     79▕
     80▕     $response->assertStatus(200);
  ➜  81▕     $response->assertViewIs('partials.thread-list');
     82▕     $response->assertSee('Project 2 Thread');
     83▕     $response->assertDontSee('Project 1 Thread');
     84▕ });

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\HomepageChatTest > chat page loads correctly after sending message
  Expected: <!DOCTYPE html>\n
  <html lang="en" class="dark">\n
  \n
  ... (345 more lines)

  To contain: Another test message...

  at tests/Feature/HomepageChatTest.php:63
     59▕
     60▕         $chatResponse = $this->actingAs($user)->get("/chat/{$thread->id}");
     61▕         $chatResponse->assertStatus(200);
     62▕         $chatResponse->assertSee('Another test message');
  ➜  63▕         $chatResponse->assertSee($thread->title);
     64▕         $chatResponse->assertSee('Send'); // Assuming there's a "Send" button on the chat page
     65▕     }
     66▕ }
     67▕

  ───────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\LoadTeamsAndProjectsTest > initial page load does not contain teams and pr…
  Expected: <!DOCTYPE html>\n
  <html lang="en" class="dark">\n
  \n
  ... (329 more lines)

  Not to contain: Team 1

  at tests/Feature/LoadTeamsAndProjectsTest.php:37
     33▕ test('initial page load does not contain teams and projects', function () {
     34▕     $response = $this->actingAs($this->user)->get(route('dashboard'));
     35▕
     36▕     $response->assertStatus(200);
  ➜  37▕     $response->assertDontSee('Team 1');
     38▕     $response->assertDontSee('Team 2');
     39▕     $response->assertDontSee('Project 1');
     40▕     $response->assertDontSee('Project 2');
