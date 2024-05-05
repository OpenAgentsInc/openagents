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
        Schema::create('codebases', function (Blueprint $table) {
            $table->id();
            $table->string('repository');
            $table->string('remote');
            $table->string('branch');
            $table->boolean('private');
            $table->string('status');
            $table->integer('files_processed');
            $table->integer('num_files');
            $table->json('sample_questions')->nullable();
            $table->string('sha');
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('codebases');
    }
};
