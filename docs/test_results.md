 PASS  Tests\Unit\MessageTest
  ✓ a message belongs to a user                                                                                                      0.09s
  ✓ a message belongs to a thread                                                                                                    0.01s
  ✓ a message can be created by the system                                                                                           0.01s

   WARN  Tests\Unit\ProjectTest
  ✓ a project belongs to a user                                                                                                      0.01s
  ✓ a project belongs to a team
  ✓ a project has many threads                                                                                                       0.01s
  ✓ a project belongs to either a user or a team
  ✓ a project has many files                                                                                                         0.01s
  - a file can be uploaded and associated with a project                                                                             0.01s

   PASS  Tests\Unit\TeamTest
  ✓ a team can have many users                                                                                                       0.01s
  ✓ a user can be a member of multiple teams                                                                                         0.01s
  ✓ a team has many projects
  ✓ a team has many threads through projects                                                                                         0.01s
  ✓ a user can have a current team
  ✓ a team can have many users with it as their current team

   PASS  Tests\Unit\ThreadTest
  ✓ a thread belongs to a user                                                                                                       0.01s
  ✓ a thread belongs to a project
  ✓ a thread has many messages                                                                                                       0.01s
  ✓ a thread belongs to a team through a project                                                                                     0.01s

   PASS  Tests\Unit\UserTest
  ✓ a user can belong to multiple teams                                                                                              0.01s
  ✓ a user can have a current team
  ✓ a user can have a null current team for personal context
  ✓ a user has many projects
  ✓ a user has many threads
  ✓ a user has many messages                                                                                                         0.01s
  ✓ a user can have projects through their current team

   PASS  Tests\Feature\Auth\AuthenticationTest
  ✓ login screen can be rendered                                                                                                     0.03s
  ✓ users can authenticate using the login screen                                                                                    0.02s
  ✓ users can not authenticate with invalid password                                                                                 0.21s
  ✓ users can logout                                                                                                                 0.01s

   PASS  Tests\Feature\Auth\EmailVerificationTest
  ✓ email verification screen can be rendered                                                                                        0.01s
  ✓ email can be verified                                                                                                            0.01s
  ✓ email is not verified with invalid hash                                                                                          0.01s

   PASS  Tests\Feature\Auth\PasswordConfirmationTest
  ✓ confirm password screen can be rendered                                                                                          0.01s
  ✓ password can be confirmed                                                                                                        0.01s
  ✓ password is not confirmed with invalid password                                                                                  0.21s

   WARN  Tests\Feature\Auth\PasswordResetTest
  - reset password link screen can be rendered                                                                                       0.01s
  - reset password link can be requested
  - reset password screen can be rendered
  - password can be reset with valid token

   PASS  Tests\Feature\Auth\PasswordUpdateTest
  ✓ password can be updated                                                                                                          0.02s
  ✓ correct password must be provided to update password                                                                             0.01s

   WARN  Tests\Feature\Auth\RegistrationTest
  - registration screen can be rendered                                                                                              0.01s
  - new users can register

   PASS  Tests\Feature\ChatRedirectTest
  ✓ visiting /chat redirects to appropriate thread                                                                                   0.01s

   PASS  Tests\Feature\ComponentLibraryTest
  ✓ it returns a successful response                                                                                                 0.01s

   PASS  Tests\Feature\IngestTest
  ✓ can ingest pdf                                                                                                                   0.05s

   PASS  Tests\Feature\InquireTest
  ✓ inquire page is displayed                                                                                                        0.01s
  ✓ inquiry can be submitted                                                                                                         0.01s
  ✓ inquiry requires valid email                                                                                                     0.01s
  ✓ inquiry requires comment with minimum length                                                                                     0.01s
  ✓ inquiry requires both email and comment                                                                                          0.01s
  ✓ successful inquiry submission shows success message                                                                              0.01s
  ✓ inquiry requires valid inquiry type

   PASS  Tests\Feature\ProfileTest
  ✓ profile page is displayed                                                                                                        0.01s
  ✓ profile information can be updated                                                                                               0.01s
  ✓ email verification status is unchanged when the email address is unchanged                                                       0.01s
  ✓ user can delete their account                                                                                                    0.01s
  ✓ correct password must be provided to delete account                                                                              0.01s

  Tests:    7 skipped, 52 passed (139 assertions)
  Duration: 1.01s
