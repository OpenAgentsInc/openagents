 FAIL  Tests\Unit\CRM\Models\ContactTest
  ✓ contact belongs to a company                                                         0.09s
  ✓ contact can belong to teams                                                          0.01s
  ⨯ contact has many activities                                                          0.01s
  ⨯ contact has many chat threads                                                        0.01s
  ⨯ contact has many notes                                                               0.01s
  ⨯ contact has many tags                                                                0.01s
  ⨯ contact calculates engagement score                                                  0.01s
  ✓ contact requires email field                                                         0.01s
  ✓ contact formats phone numbers                                                        0.01s
  ✓ contact generates unique contact ids                                                 0.01s
  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many activities     QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: activities.user_id (Connection: sqlite, SQL: insert into "activities" ("contact_id", "updated_at", "created_at") values (1, 2024-10-31 04:33:40, 2024-10-31 04:33:40))

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
  17  tests/Unit/CRM/Models/ContactTest.php:34

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many chat threads            Error
  Class "App\Models\CRM\Thread" not found

  at vendor/laravel/framework/src/Illuminate/Database/Eloquent/Concerns/HasRelationships.php:855
    851▕      * @return mixed
    852▕      */
    853▕     protected function newRelatedInstance($class)
    854▕     {
  ➜ 855▕         return tap(new $class, function ($instance) {
    856▕             if (! $instance->getConnectionName()) {
    857▕                 $instance->setConnection($this->connection);
    858▕             }
    859▕         });

      +2 vendor frames
  3   app/Models/CRM/Contact.php:68
  4   tests/Unit/CRM/Models/ContactTest.php:43

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many notes          QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: notes.user_id (Connection: sqlite, SQL: insert into "notes" ("contact_id", "updated_at", "created_at") values (1, 2024-10-31 04:33:40, 2024-10-31 04:33:40))

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
  17  tests/Unit/CRM/Models/ContactTest.php:49

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many tags           QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: tags.name (Connection: sqlite, SQL: insert into "tags" ("company_id", "updated_at", "created_at") values (1, 2024-10-31 04:33:40, 2024-10-31 04:33:40))

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
  17  tests/Unit/CRM/Models/ContactTest.php:57

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact calculates engagement…  QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: activities.user_id (Connection: sqlite, SQL: insert into "activities" ("contact_id", "updated_at", "created_at") values (1, 2024-10-31 04:33:40, 2024-10-31 04:33:40))

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
  17  tests/Unit/CRM/Models/ContactTest.php:66


  Tests:    5 failed, 5 passed (5 assertions)
