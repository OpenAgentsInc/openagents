<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('runs', function (Blueprint $table) {
            $table->string('id', 36)->primary();
            $table->string('thread_id', 36)->index();
            $table->foreignId('user_id')->index();

            $table->string('status', 32)->index();

            $table->string('model_provider')->nullable();
            $table->string('model')->nullable();
            $table->json('usage')->nullable();
            $table->json('meta')->nullable();
            $table->text('error')->nullable();

            $table->timestamp('started_at')->nullable();
            $table->timestamp('completed_at')->nullable();
            $table->timestamps();

            $table->index(['thread_id', 'created_at']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('runs');
    }
};
