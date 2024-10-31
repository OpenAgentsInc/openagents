
   FAIL  Tests\Unit\CRM\Models\ContactTest
  ✓ contact belongs to a company                                                         0.24s
  ✓ contact can optionally belong to teams                                               0.01s
  ✓ contact has many activities                                                          0.01s
  ⨯ contact has many chat threads                                                        0.01s
  ⨯ contact has many notes                                                               0.01s
  ✓ contact has many tags                                                                0.01s
  ⨯ contact calculates engagement score                                                  0.01s
  ✓ contact requires email field                                                         0.01s
  ✓ contact formats phone numbers                                                        0.01s
  ✓ contact generates unique contact ids                                                 0.01s
  ✓ contact belongs to company and not directly to team                                  0.01s
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
  4   tests/Unit/CRM/Models/ContactTest.php:51

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact has many notes                   Error
  Call to a member function first() on string

  at tests/Unit/CRM/Models/ContactTest.php:61
     57▕     Note::factory()->create([
     58▕         'contact_id' => $this->contact->id,
     59▕     ]);
     60▕
  ➜  61▕     expect($this->contact->notes->first())->toBeInstanceOf(Note::class);
     62▕ });
     63▕
     64▕ test('contact has many tags', function () {
     65▕     $tag = Tag::factory()->create([

  1   tests/Unit/CRM/Models/ContactTest.php:61

  ────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\CRM\Models\ContactTest > contact calculates engagement score      Error
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
  4   tests/Unit/CRM/Models/ContactTest.php:80


  Tests:    3 failed, 8 passed (10 assertions)
