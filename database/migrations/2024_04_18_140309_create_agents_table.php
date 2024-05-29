w<?php

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
        Schema::create('agents', function (Blueprint $table) {
            $table->id();
            $table->foreignId('user_id')->nullable()->constrained('users');
            $table->string('name');
            $table->json('image')->nullable();
            $table->text('about')->nullable();
            $table->text('message')->nullable();
            $table->longText('prompt')->nullable();
            $table->integer('sats_per_message')->default(3);
            $table->boolean('use_tools')->default(false);
            $table->text('rag_prompt')->nullable();
            $table->boolean('is_public')->default(true);
            $table->boolean('featured')->default(false);
            $table->json('capabilities')->nullable();
            $table->boolean('is_rag_ready')->default(true);
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('agents');
    }
};
