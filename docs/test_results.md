
   FAIL  Tests\Unit\CRM\Models\ContactTest
  ✓ contact belongs to a company                                                         0.21s
  ✓ contact can optionally belong to teams                                               0.01s
  ⨯ contact has many activities                                                          0.01s
  ⨯ contact has many chat threads                                                        0.01s
  ⨯ contact has many notes                                                               0.01s
  ⨯ contact has many tags                                                                0.01s
  ⨯ contact has many deals
  ⨯ contact calculates engagement score
  ✓ contact requires email field                                                         0.01s
  ✓ contact formats phone numbers                                                        0.01s
  ✓ contact generates unique contact ids                                                 0.01s
  ✓ contact belongs to company and not directly to team                                  0.01s
  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many activities     QueryException
  SQLSTATE[HY000]: General error: 1 table activities has no column named company_id (Connection: sqlite, SQL: insert into "activities" ("contact_id", "company_id", "updated_at", "created_at") values (1, 1, 2024-10-31 04:35:19, 2024-10-31 04:35:19))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:565
    561▕             if ($this->pretending()) {
    562▕                 return true;
    563▕             }
    564▕
  ➜ 565▕             $statement = $this->getPdo()->prepare($query);
    566▕
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();

      +16 vendor frames
  17  tests/Unit/CRM/Models/ContactTest.php:43

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
  4   tests/Unit/CRM/Models/ContactTest.php:53

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many notes          QueryException
  SQLSTATE[HY000]: General error: 1 table notes has no column named company_id (Connection: sqlite, SQL: insert into "notes" ("contact_id", "company_id", "updated_at", "created_at") values (1, 1, 2024-10-31 04:35:19, 2024-10-31 04:35:19))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:565
    561▕             if ($this->pretending()) {
    562▕                 return true;
    563▕             }
    564▕
  ➜ 565▕             $statement = $this->getPdo()->prepare($query);
    566▕
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();

      +16 vendor frames
  17  tests/Unit/CRM/Models/ContactTest.php:61

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many tags           QueryException
  SQLSTATE[23000]: Integrity constraint violation: 19 NOT NULL constraint failed: tags.name (Connection: sqlite, SQL: insert into "tags" ("company_id", "updated_at", "created_at") values (1, 2024-10-31 04:35:19, 2024-10-31 04:35:19))

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
  17  tests/Unit/CRM/Models/ContactTest.php:70

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many deals                   Error
  Class "App\Models\CRM\Deal" not found

  at tests/Unit/CRM/Models/ContactTest.php:79
     75▕     expect($this->contact->tags->first())->toBeInstanceOf(Tag::class);
     76▕ });
     77▕
     78▕ test('contact has many deals', function () {
  ➜  79▕     $deal = Deal::factory()->create([
     80▕         'company_id' => $this->company->id,
     81▕     ]);
     82▕     $this->contact->deals()->attach($deal->id);
     83▕

  1   tests/Unit/CRM/Models/ContactTest.php:79

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact calculates engagement…  QueryException
  SQLSTATE[HY000]: General error: 1 table activities has no column named company_id (Connection: sqlite, SQL: insert into "activities" ("contact_id", "company_id", "updated_at", "created_at") values (1, 1, 2024-10-31 04:35:19, 2024-10-31 04:35:19))

  at vendor/laravel/framework/src/Illuminate/Database/Connection.php:565
    561▕             if ($this->pretending()) {
    562▕                 return true;
    563▕             }
    564▕
  ➜ 565▕             $statement = $this->getPdo()->prepare($query);
    566▕
    567▕             $this->bindValues($statement, $this->prepareBindings($bindings));
    568▕
    569▕             $this->recordsHaveBeenModified();

      +16 vendor frames
  17  tests/Unit/CRM/Models/ContactTest.php:88
