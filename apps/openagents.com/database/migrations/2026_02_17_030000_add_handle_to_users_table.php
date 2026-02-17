<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('users', function (Blueprint $table): void {
            $table->string('handle', 64)->nullable()->after('email');
        });

        $existing = DB::table('users')
            ->select(['id', 'email', 'name'])
            ->orderBy('id')
            ->get();

        $used = [];

        foreach ($existing as $row) {
            $base = $this->baseHandleFromIdentity((string) ($row->email ?: $row->name ?: ('user'.$row->id)));

            if ($base === '') {
                $base = 'user'.$row->id;
            }

            $handle = $base;
            $suffix = 1;

            while (in_array($handle, $used, true) || DB::table('users')->where('handle', $handle)->where('id', '!=', $row->id)->exists()) {
                $suffixText = '-'.$suffix;
                $trimmedBase = substr($base, 0, max(1, 64 - strlen($suffixText)));
                $handle = $trimmedBase.$suffixText;
                $suffix++;
            }

            DB::table('users')->where('id', $row->id)->update(['handle' => $handle]);
            $used[] = $handle;
        }

        Schema::table('users', function (Blueprint $table): void {
            $table->unique('handle');
        });
    }

    public function down(): void
    {
        Schema::table('users', function (Blueprint $table): void {
            $table->dropUnique(['handle']);
            $table->dropColumn('handle');
        });
    }

    private function baseHandleFromIdentity(string $value): string
    {
        $candidate = strtolower(trim($value));

        if (str_contains($candidate, '@')) {
            $candidate = (string) explode('@', $candidate, 2)[0];
        }

        $candidate = preg_replace('/[^a-z0-9:_-]+/', '-', $candidate) ?? '';
        $candidate = trim($candidate, '-');

        if ($candidate === '') {
            return '';
        }

        return substr($candidate, 0, 64);
    }
};
