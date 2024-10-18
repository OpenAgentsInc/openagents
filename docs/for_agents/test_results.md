  PASS  Tests\Unit\MessageTest
  ✓ a message belongs to a user                                                             0.08s
  ✓ a message belongs to a thread
  ✓ a message can be created by the system

   PASS  Tests\Unit\ProjectTest
  ✓ a project belongs to a user
  ✓ a project belongs to a team
  ✓ a project has many threads                                                              0.01s
  ✓ a project belongs to either a user or a team

   PASS  Tests\Unit\TeamTest
  ✓ a team has many users                                                                   0.01s
  ✓ a team has many projects
  ✓ a team has many threads through projects

   PASS  Tests\Unit\ThreadTest
  ✓ a thread belongs to a user                                                              0.01s
  ✓ a thread belongs to a project
  ✓ a thread has many messages                                                              0.01s
  ✓ a thread belongs to a team through a project

   PASS  Tests\Unit\UserTest
  ✓ a user belongs to a team                                                                0.01s
  ✓ a user has many projects
  ✓ a user has many threads                                                                 0.01s
  ✓ a user has many messages                                                                0.01s
  ✓ a user can have projects through their team                                             0.01s

   PASS  Tests\Feature\Auth\AuthenticationTest
  ✓ login screen can be rendered                                                            0.05s
  ✓ users can authenticate using the login screen                                           0.02s
  ✓ users can not authenticate with invalid password                                        0.22s
  ✓ users can logout                                                                        0.01s

   PASS  Tests\Feature\Auth\EmailVerificationTest
  ✓ email verification screen can be rendered                                               0.01s
  ✓ email can be verified                                                                   0.01s
  ✓ email is not verified with invalid hash                                                 0.01s

   PASS  Tests\Feature\Auth\PasswordConfirmationTest
  ✓ confirm password screen can be rendered                                                 0.01s
  ✓ password can be confirmed                                                               0.01s
  ✓ password is not confirmed with invalid password                                         0.21s

   PASS  Tests\Feature\Auth\PasswordResetTest
  ✓ reset password link screen can be rendered                                              0.01s
  ✓ reset password link can be requested                                                    0.01s
  ✓ reset password screen can be rendered                                                   0.01s
  ✓ password can be reset with valid token                                                  0.01s

   PASS  Tests\Feature\Auth\PasswordUpdateTest
  ✓ password can be updated                                                                 0.01s
  ✓ correct password must be provided to update password                                    0.01s

   PASS  Tests\Feature\Auth\RegistrationTest
  ✓ registration screen can be rendered                                                     0.01s
  ✓ new users can register                                                                  0.01s

   PASS  Tests\Feature\ComponentLibraryTest
  ✓ component library loads component library view                                          0.01s

   PASS  Tests\Feature\CoreFunctionalityTest
  ✓ user can send a message in a thread                                                     0.01s
  ✓ system can add a message to a thread                                                    0.01s
  ✓ threads can be organized into projects                                                  0.01s
  ✓ threads can be organized into teams                                                     0.01s
  ✓ system can make LLM tool calls with GitHub API                                          0.01s

   FAIL  Tests\Feature\HomepageChatTest
  ⨯ authenticated user can send a message from homepage and is redirected to new chat thre… 0.03s
  ✓ unauthenticated user is redirected to login when trying to send a message from homepage

   PASS  Tests\Feature\HomepageTest
  ✓ homepage loads homepage view for unauthenticated users                                  0.01s
  ✓ homepage loads dashboard view for authenticated users                                   0.01s

   PASS  Tests\Feature\SendMessageTest
  ✓ authenticated user can send a message without a project                                 0.01s
  ✓ authenticated user can send a message with a project                                    0.01s
  ✓ authenticated user can send a message to an existing thread                             0.01s
  ✓ unauthenticated user cannot send a message
  ✓ message cannot be empty                                                                 0.01s
  ✓ project_id must be valid if provided
  ───────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Feature\HomepageChatTest > authenticated user can send a message from homepage…
  Expected response status code [302] but received 201.
Failed asserting that 201 is identical to 302.

  at tests/Feature/HomepageChatTest.php:17
     13▕         ->post('/send-message', [
     14▕             'message' => 'Test message from homepage'
     15▕         ]);
     16▕
  ➜  17▕     $response->assertStatus(302); // Assert that there's a redirect
     18▕
     19▕     $thread = Thread::latest()->first();
     20▕     $response->assertRedirect("/chat/{$thread->id}");
     21▕


  Tests:    1 failed, 52 passed (128 assertions)
  Duration: 0.99s
