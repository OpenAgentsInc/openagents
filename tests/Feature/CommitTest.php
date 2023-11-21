<?php

$diff = "```diff
diff --git a/app/Http/Controllers/MemoriesController.php b/app/Http/Controllers/MemoriesController.php
new file mode 100644
index 00000000..c12345d8
--- /dev/null
+++ b/app/Http/Controllers/MemoriesController.php
@@ -0,0 +1,37 @@
+<?php
+
+namespace App\Http\Controllers;
+
+use Illuminate\Http\Request;
+use App\Models\Memory;
+
+class MemoriesController extends Controller
+{
+    public function create(Request $request)
+    {
+        return Memory::create($request->all());
+    }
+
+    public function read(int $id)
+    {
+        return Memory::findOrFail($id);
+    }
+
+    public function update(Request $request, int $id)
+    {
+        $memory = Memory::findOrFail($id);
+        $memory->update($request->all());
+
+        return $memory;
+    }
+
+    public function delete(int $id)
+    {
+        $memory = Memory::findOrFail($id);
+        $memory->delete();
+
+        return response()->json(null, 204);
+    }
+}
+
+```
```diff
diff --git a/routes/web.php b/routes/web.php
index c4d8e963..823efaeb 100644
--- a/routes/web.php
+++ b/routes/web.php
@@ -16,6 +16,11 @@ Route::get('/', function () {
 Route::get('/dashboard', function () {
     return view('dashboard');
 })->middleware(['auth'])->name('dashboard');
+
+Route::post('/memories', 'MemoriesController@create');
+Route::get('/memories/{id}', 'MemoriesController@read');
+Route::put('/memories/{id}', 'MemoriesController@update');
+Route::delete('/memories/{id}', 'MemoriesController@delete');

 require __DIR__.'/auth.php';

```
```diff
diff --git a/tests/Unit/MemoryTest.php b/tests/Unit/MemoryTest.php
index 91856494..b876c2f8 100644
--- a/tests/Unit/MemoryTest.php
+++ b/tests/Unit/MemoryTest.php
@@ -34,4 +34,34 @@ class MemoryTest extends TestCase
        $this->assertDatabaseMissing('memories', ['id' => $memory->id]);
     });

+    it('can create a memory via HTTP', function() {
+        $params = ['description' => 'New memory'];
+        $response = $this->postJson('/memories', $params);
+
+        $response->assertStatus(201)
+            ->assertJson($params);
+    });
+
+    it('can update a memory via HTTP', function() {
+        $memory = Memory::create(['description' => 'Original memory']);
+
+        $params = ['description' => 'Updated memory'];
+        $response = $this->putJson("/memories/{$memory->id}", $params);
+
+        $response->assertStatus(200)
+            ->assertJson($params);
+    });
+
+    it('can read a memory via HTTP', function() {
+        $memory = Memory::create(['description' => 'Original memory']);
+
+        $response = $this->getJson("/memories/{$memory->id}");
+
+        $response->assertStatus(200)
+            ->assertJson(['description' => 'Original memory']);
+    });
+
+    it('can delete a memory via HTTP', function() {
+        $memory = Memory::create(['description' => 'Original memory']);
+
+        $response = $this->deleteJson("/memories/{$memory->id}");
+
+        $response->assertStatus(204);
+        $this->assertDatabaseMissing('memories', ['id' => $memory->id]);
+    });
 })
```";

test("it can generate a diff", function () {
    dd($diff);
}};
