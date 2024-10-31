
   WARN  Tests\Unit\ProjectTest
  ✓ a project belongs to a user                                                                        0.20s
  ✓ a project belongs to a team                                                                        0.01s
  ✓ a project has many threads                                                                         0.01s
  ✓ a project belongs to either a user or a team                                                       0.01s
  ✓ a project has many files                                                                           0.01s
  - a file can be uploaded and associated with a project                                               0.01s
  ✓ a project can have custom instructions                                                             0.01s
  ✓ a project can have custom settings
  ✓ team members can access team project                                                               0.01s
  ✓ non-team members cannot access team project                                                        0.01s
  ✓ project requires a name                                                                            0.01s
  ✓ project name must be unique within team/user scope                                                 0.01s
  ✓ threads inherit project context and instructions                                                   0.01s
  ✓ project can be archived

   PASS  Tests\Unit\TeamTest
  ✓ a team has many projects                                                                           0.01s
  ✓ a team has many threads through projects                                                           0.01s

   PASS  Tests\Unit\ThreadTest
  ✓ a thread belongs to a project                                                                      0.01s
  ✓ a thread belongs to a team through a project                                                       0.01s
  ✓ thread inherits project context
  ✓ thread inherits project instructions
  ✓ thread returns empty context when no project
  ✓ thread returns empty instructions when no project

   PASS  Tests\Unit\UserTest
  ✓ a user has many projects                                                                           0.01s
  ✓ a user can have projects through their current team                                                0.01s

   PASS  Tests\Feature\ThreadOwnershipTest
  ✓ threads are created without project when no team is selected                                       0.04s
  ✓ threads are created with default project when team is selected
