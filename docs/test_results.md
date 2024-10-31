
   FAIL  Tests\Unit\CRM\Models\ContactTest
  ⨯ contact belongs to a company
  ⨯ contact can belong to teams
  ⨯ contact has many activities
  ⨯ contact has many chat threads
  ⨯ contact has many notes
  ⨯ contact has many tags
  ⨯ contact calculates engagement score
  ⨯ contact requires email field
  ⨯ contact formats phone numbers
  ⨯ contact generates unique contact ids
  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact belongs to a company            QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14

  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact can belong to teams             QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14

  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many activities             QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14

  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many chat threads           QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14

  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many notes                  QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14

  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many tags                   QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14

  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact calculates engagement score     QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14

  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact requires email field            QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14

  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact formats phone numbers           QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14

  ────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact generates unique contact ids    QueryException
  SQLSTATE[HY000]: General error: 1 table "activities" already exists (Connection: sqlite, SQL: create table "activities" ("id" integer primary key autoincrement not null, "created_at" datetime, "updated_at" datetime))

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

      +8 vendor frames
  9   database/migrations/2024_10_31_042822_create_activities_table.php:14


  Tests:    10 failed (0 assertions)
