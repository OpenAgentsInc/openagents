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
        Schema::create('pool_job_payment_requests', function (Blueprint $table) {
            $table->id();
            $table->timestamps();
            $table->foreignId('pool_job_id')->constrained()->onDelete('cascade');
            $table->integer('amount')->default(0);
            $table->string('protocol')->default('lightning');
            $table->string('currency')->default('bitcoin');
            $table->string('target');
            $table->boolean('paid')->default(false);
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('pool_job_payment_requests');
    }
};
