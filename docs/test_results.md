
   FAIL  Tests\Unit\CRM\Models\ContactEmailTest
  ⨯ email belongs to a contact
  ⨯ email tracks metadata
  ⨯ email handles threading
  ⨯ email manages attachments
  ⨯ email validates addresses

   FAIL  Tests\Unit\CRM\Models\ContactNoteTest
  ⨯ note belongs to a contact
  ⨯ note belongs to a user
  ⨯ note supports markdown formatting
  ⨯ note tracks edit history
  ⨯ note handles mentions
  ⨯ note requires content

   FAIL  Tests\Unit\CRM\Models\ContactTagTest
  ✓ tag belongs to a contact                                                                           0.02s
  ✓ tag belongs to a team                                                                              0.01s
  ⨯ tag enforces unique constraints                                                                    0.01s
  ⨯ tag validates format                                                                               0.01s

   FAIL  Tests\Unit\CRM\Models\ContactTest
  ✓ contact belongs to a company                                                                       0.01s
  ✓ contact can optionally belong to teams                                                             0.01s
  ✓ contact has many activities                                                                        0.01s
  ✓ contact has many chat threads                                                                      0.01s
  ✓ contact has many notes                                                                             0.01s
  ⨯ contact has many tags                                                                              0.01s
  ✓ contact calculates engagement score                                                                0.01s
  ✓ contact requires email field                                                                       0.01s
  ✓ contact formats phone numbers
  ✓ contact generates unique contact ids
  ✓ contact belongs to company and not directly to team                                                0.01s

   FAIL  Tests\Unit\CRM\Services\ContactAIServiceTest
  ⨯ ai service analyzes contact interactions
  ⨯ ai service generates contact summaries
  ⨯ ai service calculates relationship scores
  ⨯ ai service identifies action items
  ⨯ ai service suggests follow ups
  ⨯ ai service handles missing data gracefully
  ⨯ ai service respects rate limits

   FAIL  Tests\Unit\CRM\Services\ContactImportServiceTest
  ⨯ import service validates file format
  ⨯ import service maps fields correctly
  ⨯ import service detects duplicates
  ⨯ import service processes batch imports
  ⨯ import service reports errors

   FAIL  Tests\Unit\CRM\Services\ContactMergeServiceTest
  ⨯ merge service combines contact basic info
  ⨯ merge service combines activities
  ⨯ merge service combines emails
  ⨯ merge service combines notes
  ⨯ merge service handles conflict resolution
  ⨯ merge service maintains audit trail

   FAIL  Tests\Unit\CRM\Services\ContactSearchServiceTest
  ⨯ search service indexes contact data
  ⨯ search service performs fuzzy matching
  ⨯ search service ranks results
  ⨯ search service filters by permissions
  ⨯ search service optimizes query performance
  ⨯ search service handles complex criteria
  ⨯ search service provides suggestions
  ⨯ search service handles empty search gracefully

   PASS  Tests\Unit\MessageTest
  ✓ a message belongs to a user                                                                        0.01s
  ✓ a message belongs to a thread
  ✓ a message can be created by the system                                                             0.01s
  ✓ a message can have many tool invocations                                                           0.01s

   WARN  Tests\Unit\ProjectTest
  ✓ a project belongs to a user                                                                        0.01s
  ✓ a project belongs to a team                                                                        0.01s
  ✓ a project has many threads                                                                         0.01s
  ✓ a project belongs to either a user or a team                                                       0.01s
  ✓ a project has many files                                                                           0.01s
  - a file can be uploaded and associated with a project

   PASS  Tests\Unit\TeamTest
  ✓ a team can have many users                                                                         0.01s
  ✓ a user can be a member of multiple teams                                                           0.01s
  ✓ a team has many projects                                                                           0.01s
  ✓ a team has many threads through projects                                                           0.01s
  ✓ a user can have a current team                                                                     0.01s
  ✓ a team can have many users with it as their current team

   PASS  Tests\Unit\ThreadTest
  ✓ a thread belongs to a user                                                                         0.01s
  ✓ a thread belongs to a project                                                                      0.01s
  ✓ a thread has many messages                                                                         0.01s
  ✓ a thread belongs to a team through a project                                                       0.01s

   PASS  Tests\Unit\UserTest
  ✓ a user can belong to multiple teams                                                                0.01s
  ✓ a user can have a current team
  ✓ a user can have a null current team for personal context
  ✓ a user has many projects
  ✓ a user has many threads
  ✓ a user has many messages                                                                           0.01s
  ✓ a user can have projects through their current team

   PASS  Tests\Feature\Auth\AuthenticationTest
  ✓ login screen can be rendered                                                                       0.04s
  ✓ users can authenticate using the login screen                                                      0.02s
  ✓ users can not authenticate with invalid password                                                   0.21s
  ✓ users can logout                                                                                   0.01s

   PASS  Tests\Feature\Auth\EmailVerificationTest
  ✓ email verification screen can be rendered                                                          0.01s
  ✓ email can be verified                                                                              0.01s
  ✓ email is not verified with invalid hash                                                            0.01s

   PASS  Tests\Feature\Auth\PasswordConfirmationTest
  ✓ confirm password screen can be rendered                                                            0.01s
  ✓ password can be confirmed                                                                          0.01s
  ✓ password is not confirmed with invalid password                                                    0.21s

   WARN  Tests\Feature\Auth\PasswordResetTest
  - reset password link screen can be rendered                                                         0.01s
  - reset password link can be requested                                                               0.01s
  - reset password screen can be rendered                                                              0.01s
  - password can be reset with valid token                                                             0.01s

   PASS  Tests\Feature\Auth\PasswordUpdateTest
  ✓ password can be updated                                                                            0.02s
  ✓ correct password must be provided to update password                                               0.01s

   WARN  Tests\Feature\Auth\RegistrationTest
  - registration screen can be rendered                                                                0.01s
  - new users can register

   FAIL  Tests\Feature\ChatRedirectTest
  ⨯ visiting /chat redirects to users most recent thread                                               0.01s
  ✓ visiting /chat redirects to /chat/create when user has no threads                                  0.01s
  ⨯ visiting /chat/create creates a new thread                                                         0.01s

   PASS  Tests\Feature\ChatTest
  ✓ chat page shows thread messages with tool invocations                                              0.01s

   WARN  Tests\Feature\ChatThreadIdTest
  - chat messages are saved to specified thread

   PASS  Tests\Feature\ChatThreadsTest
  ✓ chat page shows user threads                                                                       0.01s

   PASS  Tests\Feature\ComponentLibraryTest
  ✓ it returns a successful response                                                                   0.01s

   WARN  Tests\Feature\ConverseStreamTest
  - converse stream works → long, skipping for now                                                     0.01s

   PASS  Tests\Feature\CreateTeamTest
  ✓ authenticated user can create a new team                                                           0.01s
  ✓ team name is required                                                                              0.01s
  ✓ team name must be unique for the creating user                                                     0.01s
  ✓ guest cannot create a team                                                                         0.01s
  ✓ user can switch between teams                                                                      0.01s
  ✓ user can switch to personal context                                                                0.01s

   PASS  Tests\Feature\DeleteThreadTest
  ✓ user can delete their own thread                                                                   0.01s
  ✓ user cannot delete another users thread                                                            0.01s
  ✓ deleting thread removes all associated messages                                                    0.01s

   PASS  Tests\Feature\IngestTest
  ✓ can ingest pdf                                                                                     0.37s

   PASS  Tests\Feature\InquireTest
  ✓ inquire page is displayed                                                                          0.01s
  ✓ inquiry can be submitted                                                                           0.01s
  ✓ inquiry requires valid email                                                                       0.01s
  ✓ inquiry requires comment with minimum length                                                       0.01s
  ✓ inquiry requires both email and comment                                                            0.01s
  ✓ successful inquiry submission shows success message                                                0.01s
  ✓ inquiry requires valid inquiry type                                                                0.01s

   PASS  Tests\Feature\ProfileTest
  ✓ profile page is displayed                                                                          0.01s
  ✓ profile information can be updated                                                                 0.01s
  ✓ email verification status is unchanged when the email address is unchanged                         0.01s
  ✓ user can delete their account                                                                      0.01s
  ✓ correct password must be provided to delete account                                                0.01s

   PASS  Tests\Feature\ThreadOwnershipTest
  ✓ threads are created without project when no team is selected                                       0.01s
  ✓ threads are created with default project when team is selected                                     0.01s
  ✓ user can view their personal threads                                                               0.01s
  ✓ user can view their team threads                                                                   0.01s
  ✓ user cannot view threads from teams they dont belong to                                            0.01s
  ✓ user cannot view other users personal threads                                                      0.01s
  ✓ threads list shows only personal threads in personal context                                       0.01s
  ✓ threads list shows only team threads in team context                                               0.01s

   WARN  Tests\Feature\UseChatToolsTest
  - chat tools response has correct format
