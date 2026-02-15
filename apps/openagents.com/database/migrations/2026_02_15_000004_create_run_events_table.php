<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('run_events', function (Blueprint $table) {
            $table->bigIncrements('id');
            $table->string('thread_id', 36)->index();
            $table->string('run_id', 36)->index();
            $table->foreignId('user_id')->index();

            $table->string('type', 64)->index();
            $table->json('payload')->nullable();

            $table->timestamp('created_at')->useCurrent();

            $table->index(['run_id', 'id']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('run_events');
    }
};
