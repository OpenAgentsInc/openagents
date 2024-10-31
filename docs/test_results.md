 openagents git:(teams) pf Team

   FAIL  Tests\Unit\CRM\Models\ContactTagTest
  ⨯ tag belongs to a team                                                                0.17s

   FAIL  Tests\Unit\CRM\Models\ContactTest
  ⨯ contact can optionally belong to teams
  ⨯ contact belongs to company and not directly to team

   PASS  Tests\Unit\ProjectTest
  ✓ a project belongs to a team                                                          0.01s
  ✓ a project belongs to either a user or a team                                         0.01s

   PASS  Tests\Unit\TeamTest
  ✓ a team can have many users                                                           0.01s
  ✓ a user can be a member of multiple teams                                             0.01s
  ✓ a team has many projects                                                             0.01s
  ✓ a team has many threads through projects                                             0.01s
  ✓ a user can have a current team                                                       0.01s
  ✓ a team can have many users with it as their current team

   PASS  Tests\Unit\ThreadTest
  ✓ a thread belongs to a team through a project                                         0.01s

   PASS  Tests\Unit\UserTest
  ✓ a user can belong to multiple teams                                                  0.01s
  ✓ a user can have a current team                                                       0.01s
  ✓ a user can have a null current team for personal context
  ✓ a user can have projects through their current team
  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTagTest > tag belongs to a team
  Failed asserting that null is an instance of class App\Models\Team.

  at tests/Unit/CRM/Models/ContactTagTest.php:25
     21▕     expect($this->contact->tags->first())->toBeInstanceOf(Tag::class);
     22▕ });
     23▕
     24▕ test('tag belongs to a team', function () {
  ➜  25▕     expect($this->tag->team)->toBeInstanceOf(Team::class);
     26▕ });
     27▕
     28▕ test('tag enforces unique constraints', function () {
     29▕     Tag::factory()->create([

  1   tests/Unit/CRM/Models/ContactTagTest.php:25

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact can optionally belong…  QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: contacts.team_id (Connection: sqlite, SQL: insert into "contacts" ("contact_id", "company_id", "created_by", "first_name", "last_name", "email", "phone", "title", "updated_at", "created_at") values (CT907359, 1, 1, Enoch, Thompson, pietro.bechtelar@example.com, 5294814960, Rail Yard Engineer, 2024-10-31 15:05:22, 2024-10-31 15:05:22))

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
  17  tests/Unit/CRM/Models/ContactTest.php:16

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact belongs to company an…  QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: contacts.team_id (Connection: sqlite, SQL: insert into "contacts" ("contact_id", "company_id", "created_by", "first_name", "last_name", "email", "phone", "title", "updated_at", "created_at") values (CT525310, 1, 1, Fae, Mante, isaiah.goldner@example.org, 6957292087, Press Machine Setter, Operator, 2024-10-31 15:05:22, 2024-10-31 15:05:22))

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
  17  tests/Unit/CRM/Models/ContactTest.php:16


  Tests:    3 failed, 13 passed (28 assertions)
