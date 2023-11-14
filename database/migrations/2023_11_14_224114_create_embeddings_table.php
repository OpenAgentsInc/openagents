<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('embeddings', function (Blueprint $table) {
            $table->id();
            $table->vector('embedding', 768);
            $table->json('metadata');
            $table->timestamps();
        });

        // This is a Postgres-specific index that allows us to do fast nearest-neighbor searches
        // when there are a lot of high-dimensional embeddings in the database.
        DB::statement('CREATE INDEX my_index ON embeddings USING ivfflat (embedding vector_l2_ops) WITH (lists = 100)');
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('embeddings');
    }
};
