<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('messages', function (Blueprint $table) {
            $table->string('id', 36)->primary();
            $table->string('thread_id', 36)->index();
            $table->string('run_id', 36)->nullable()->index();
            $table->foreignId('user_id')->index();

            $table->string('role', 25);
            $table->text('content');
            $table->json('meta')->nullable();

            $table->timestamps();

            $table->index(['thread_id', 'created_at']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('messages');
    }
};
