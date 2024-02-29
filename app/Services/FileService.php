<?php

namespace App\Services;

use App\Models\File;
use Exception;
use Illuminate\Database\Eloquent\Collection;
use Illuminate\Support\Facades\Auth;

class FileService
{
    /**
     * Retrieves all files owned by the currently authenticated user.
     *
     * This method can be expanded to include pagination or filtering
     * based on application requirements.
     *
     * @return Collection|static[]
     */
    public function getAllFilesByUser()
    {
        $userId = Auth::id(); // Get the currently authenticated user's ID

        return File::where('user_id', $userId)->get();
    }

    /**
     * Creates a new file with the given details.
     *
     * @param  string  $description  A brief description of the file.
     * @param  string  $path  The file path.
     * @param  int  $agentId  The ID of the associated agent.
     * @return File The created File object.
     *
     * @throws Exception
     */
    public function createFile(string $description, string $path, int $agentId): File
    {
        // Assuming 'user_id' is required to associate a file with a user.
        // Ensure the user is authenticated before creating a file.
        if (! Auth::check()) {
            throw new Exception('User must be authenticated to create a file.');
        }

        // Create and return the new file.
        return File::create([
            'description' => $description,
            'path' => $path,
            'agent_id' => $agentId,
            'user_id' => Auth::id(), // Or another way to obtain the current user's ID, depending on your auth system
        ]);
    }

    /**
     * Finds a file by its ID.
     *
     * @param  int|string  $id  The ID of the file to find.
     * @return File|null The found file or null if not found.
     */
    public function findFileById($id): ?File
    {
        return File::find($id);
    }

    /**
     * Updates a file with the given details.
     *
     * @param  int|string  $id  The ID of the file to update.
     * @param  array  $data  The data to update the file with.
     * @return File|null The updated file object or null if the update failed.
     */
    public function updateFile($id, array $data): ?File
    {
        $file = File::find($id);
        if (! $file) {
            return null;
        }

        $file->update($data);

        return $file;
    }

    /**
     * Deletes a file by its ID.
     *
     * @param  int|string  $id  The ID of the file to delete.
     * @return bool True if the file was deleted successfully, false otherwise.
     */
    public function deleteFile($id): bool
    {
        $file = File::find($id);
        if (! $file) {
            return false;
        }

        return $file->delete();
    }
}
