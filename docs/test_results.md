  PASS  Tests\Unit\CRM\Models\ContactTagTest
  ✓ tag belongs to a team                                                                              0.19s

   PASS  Tests\Unit\CRM\Models\ContactTest
  ✓ contact can optionally belong to teams                                                             0.02s
  ✓ contact belongs to company and not directly to team                                                0.01s

   PASS  Tests\Unit\ProjectTest
  ✓ a project belongs to a team                                                                        0.01s
  ✓ a project belongs to either a user or a team                                                       0.01s

   PASS  Tests\Unit\TeamTest
  ✓ a team can have many users                                                                         0.01s
  ✓ a user can be a member of multiple teams                                                           0.01s
  ✓ a team has many projects                                                                           0.01s
  ✓ a team has many threads through projects                                                           0.01s
  ✓ a user can have a current team                                                                     0.01s
  ✓ a team can have many users with it as their current team

   PASS  Tests\Unit\ThreadTest
  ✓ a thread belongs to a team through a project                                                       0.01s

   PASS  Tests\Unit\UserTest
  ✓ a user can belong to multiple teams                                                                0.01s
  ✓ a user can have a current team                                                                     0.01s
  ✓ a user can have a null current team for personal context
  ✓ a user can have projects through their current team                                                0.01s

  Tests:    16 passed (32 assertions)
