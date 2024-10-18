  PASS  Tests\Unit\MessageTest
  ✓ a message belongs to a user                                                             0.15s
  ✓ a message belongs to a thread
  ✓ a message can be created by the system

   PASS  Tests\Unit\ProjectTest
  ✓ a project belongs to a user                                                             0.01s
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
  ✓ a user has many projects                                                                0.01s
  ✓ a user has many threads
  ✓ a user has many messages                                                                0.01s
  ✓ a user can have projects through their team                                             0.01s

   PASS  Tests\Feature\Auth\AuthenticationTest
  ✓ login screen can be rendered                                                            0.03s
  ✓ users can authenticate using the login screen                                           0.02s
  ✓ users can not authenticate with invalid password                                        0.21s
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

   PASS  Tests\Feature\HomepageTest
  ✓ homepage loads homepage view for unauthenticated users                                  0.01s
  ✓ homepage loads dashboard view for authenticated users                                   0.01s

   PASS  Tests\Feature\SendMessageTest
  ✓ authenticated user can send a message without a project                                 0.01s
  ✓ authenticated user can send a message with a project                                    0.01s
  ✓ authenticated user can send a message to an existing thread                             0.01s
  ✓ unauthenticated user cannot send a message
  ✓ message cannot be empty
  ✓ project_id must be valid if provided

  Tests:    51 passed (124 assertions)
  Duration: 1.05s
