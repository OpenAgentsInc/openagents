<?php

namespace App\Support\ConvexImport;

use ZipArchive;

/**
 * Reads Convex snapshot exports in ZIP format.
 *
 * Expected format:
 * - one directory per table
 * - each table directory contains documents.jsonl
 *
 * Example:
 *   users/documents.jsonl
 *   threads/documents.jsonl
 */
final class ConvexExportReader
{
    private ?ZipArchive $zip = null;

    /**
     * @var array<string, string> tableName => zip entry path
     */
    private array $zipTableEntries = [];

    public function __construct(private readonly string $sourcePath)
    {
        if (is_file($sourcePath)) {
            $zip = new ZipArchive;
            $result = $zip->open($sourcePath);

            if ($result === true) {
                $this->zip = $zip;
                $this->indexZipTableEntries($zip);
            }
        }
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    public function readTable(string $tableName): array
    {
        return $this->zip
            ? $this->readTableFromZip($tableName)
            : $this->readTableFromDirectory($tableName);
    }

    public function __destruct()
    {
        if ($this->zip instanceof ZipArchive) {
            $this->zip->close();
        }
    }

    private function indexZipTableEntries(ZipArchive $zip): void
    {
        for ($i = 0; $i < $zip->numFiles; $i++) {
            $name = (string) $zip->getNameIndex($i);

            if (! str_ends_with($name, '/documents.jsonl')) {
                continue;
            }

            $segments = explode('/', trim($name, '/'));
            $count = count($segments);

            if ($count < 2) {
                continue;
            }

            // .../<table>/documents.jsonl
            $table = $segments[$count - 2] ?? null;
            if (! is_string($table) || $table === '') {
                continue;
            }

            $this->zipTableEntries[$table] = $name;
        }
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    private function readTableFromZip(string $tableName): array
    {
        $entry = $this->zipTableEntries[$tableName] ?? null;
        if (! is_string($entry) || $entry === '') {
            return [];
        }

        $stream = $this->zip?->getStream($entry);
        if (! is_resource($stream)) {
            return [];
        }

        $rows = [];

        while (($line = fgets($stream)) !== false) {
            $line = trim($line);
            if ($line === '') {
                continue;
            }

            $decoded = json_decode($line, true);
            if (is_array($decoded)) {
                $rows[] = $decoded;
            }
        }

        fclose($stream);

        return $rows;
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    private function readTableFromDirectory(string $tableName): array
    {
        $file = $this->resolveDocumentsFileInDirectory($tableName);
        if (! is_string($file) || $file === '') {
            return [];
        }

        $handle = fopen($file, 'rb');
        if (! is_resource($handle)) {
            return [];
        }

        $rows = [];

        while (($line = fgets($handle)) !== false) {
            $line = trim($line);
            if ($line === '') {
                continue;
            }

            $decoded = json_decode($line, true);
            if (is_array($decoded)) {
                $rows[] = $decoded;
            }
        }

        fclose($handle);

        return $rows;
    }

    private function resolveDocumentsFileInDirectory(string $tableName): ?string
    {
        if (! is_dir($this->sourcePath)) {
            return null;
        }

        $direct = rtrim($this->sourcePath, '/').'/'.$tableName.'/documents.jsonl';
        if (is_file($direct)) {
            return $direct;
        }

        $iterator = new \RecursiveIteratorIterator(
            new \RecursiveDirectoryIterator($this->sourcePath, \FilesystemIterator::SKIP_DOTS),
        );

        $needle = '/'.$tableName.'/documents.jsonl';

        foreach ($iterator as $fileInfo) {
            if (! $fileInfo instanceof \SplFileInfo || ! $fileInfo->isFile()) {
                continue;
            }

            $pathname = str_replace('\\', '/', $fileInfo->getPathname());
            if (str_ends_with($pathname, $needle)) {
                return $fileInfo->getPathname();
            }
        }

        return null;
    }
}
