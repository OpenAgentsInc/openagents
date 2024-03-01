<?php

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Services\FileService;
use Illuminate\Http\Request;

class FileController extends Controller
{
    protected FileService $fileService;

    public function __construct(FileService $fileService)
    {
        $this->fileService = $fileService;
    }

    /**
     * @OA\Get(
     *     path="/files",
     *     tags={"File"},
     *     summary="List files",
     *     description="Returns a list of all files.",
     *     operationId="listFiles",
     *
     *     @OA\Response(
     *         response=200,
     *         description="Successful operation",
     *
     *         @OA\JsonContent(
     *             type="array",
     *
     *             @OA\Items(ref="#/components/schemas/File")
     *         )
     *     ),
     *     security={{"bearerAuth":{}}}
     * )
     */
    public function index()
    {
        try {
            // Fetch all files using the file service
            $files = $this->fileService->getAllFilesByUser();

            // Return the list of files with a success message
            return response()->json([
                'success' => true,
                'data' => $files,
            ], 200);
        } catch (Exception $e) {
            // Handle any exceptions, such as database errors
            return response()->json(['success' => false, 'message' => $e->getMessage()], 500);
        }
    }

    /**
     * @OA\Post(
     *     path="/files",
     *     tags={"File"},
     *     summary="Upload file",
     *     description="Store a new file.",
     *     operationId="storeFile",
     *
     *     @OA\RequestBody(
     *         required=true,
     *         description="File information",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="description", type="string"),
     *             @OA\Property(property="path", type="string"),
     *             @OA\Property(property="agent_id", type="integer")
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=201,
     *         description="File created",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *             @OA\Property(
     *                 property="data",
     *                 type="object",
     *                 @OA\Property(property="file_id", type="integer")
     *             )
     *         )
     *     ),
     *     security={{"bearerAuth":{}}}
     * )
     */
    public function store(Request $request)
    {
        // Validate the incoming request without 'name'
        $validated = $request->validate([
            'description' => 'required|string',
            'path' => 'required|string',
        ]);

        try {
            // Create a new file using the file service without 'name'
            $file = $this->fileService->createFile(
                $validated['description'],
                $validated['path'],
            );

            // Return the created file with a success message
            return response()->json([
                'success' => true,
                'message' => 'File created successfully.',
                'data' => [
                    'file_id' => $file->id,
                ],
            ], 201);
        } catch (Exception $e) {
            // Handle any exceptions, such as database errors
            return response()->json(['success' => false, 'message' => $e->getMessage()], 500);
        }
    }

    /**
     * @OA\Get(
     *     path="/files/{id}",
     *     tags={"File"},
     *     summary="Retrieve file",
     *     description="Returns a single file.",
     *     operationId="getFileById",
     *
     *     @OA\Parameter(
     *         name="id",
     *         in="path",
     *         description="ID of file to return",
     *         required=true,
     *
     *         @OA\Schema(
     *             type="integer"
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=200,
     *         description="Successful operation",
     *
     *         @OA\JsonContent(ref="#/components/schemas/File")
     *     ),
     *     security={{"bearerAuth":{}}}
     * )
     */
    public function show($id)
    {
        try {
            // Find the file by ID using the file service
            $file = $this->fileService->findFileById($id);

            // Return the found file with a success message
            return response()->json([
                'success' => true,
                'data' => $file,
            ], 200);
        } catch (Exception $e) {
            // Handle any exceptions, such as database errors
            return response()->json(['success' => false, 'message' => $e->getMessage()], 500);
        }
    }

    /**
     * @OA\Put(
     *     path="/files/{id}",
     *     tags={"File"},
     *     summary="Modify file",
     *     description="Modifies a file.",
     *     operationId="updateFile",
     *
     *     @OA\Parameter(
     *         name="id",
     *         in="path",
     *         description="ID of the file to update",
     *         required=true,
     *
     *         @OA\Schema(
     *             type="integer"
     *         )
     *     ),
     *
     *     @OA\RequestBody(
     *         required=true,
     *         description="File information",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="name", type="string"),
     *             @OA\Property(property="description", type="string"),
     *             @OA\Property(property="path", type="string"),
     *             @OA\Property(property="agent_id", type="integer")
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=200,
     *         description="File updated",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *             @OA\Property(
     *                 property="data",
     *                 type="object",
     *                 ref="#/components/schemas/File"
     *             )
     *         )
     *     ),
     *     security={{"bearerAuth":{}}}
     * )
     */
    public function update(Request $request, $id)
    {
        // Validate the incoming request without 'name'
        $validated = $request->validate([
            'description' => 'sometimes|required|string',
            'path' => 'sometimes|required|string',
            'agent_id' => 'sometimes|required|integer',
        ]);

        try {
            // Update the file using the file service without 'name'
            $file = $this->fileService->updateFile($id, $validated);

            // Return the updated file with a success message
            return response()->json([
                'success' => true,
                'message' => 'File updated successfully.',
                'data' => $file,
            ], 200);
        } catch (Exception $e) {
            // Handle any exceptions, such as database errors
            return response()->json(['success' => false, 'message' => $e->getMessage()], 500);
        }
    }

    /**
     * @OA\Delete(
     *     path="/files/{id}",
     *     tags={"File"},
     *     summary="Delete file",
     *     description="Deletes a file.",
     *     operationId="deleteFile",
     *
     *     @OA\Parameter(
     *         name="id",
     *         in="path",
     *         description="ID of the file to delete",
     *         required=true,
     *
     *         @OA\Schema(
     *             type="integer"
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=200,
     *         description="File deleted",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string")
     *         )
     *     ),
     *     security={{"bearerAuth":{}}}
     * )
     */
    public function destroy($id)
    {
        try {
            // Delete the file using the file service
            $this->fileService->deleteFile($id);

            // Return a success message
            return response()->json([
                'success' => true,
                'message' => 'File deleted successfully.',
            ], 200);
        } catch (Exception $e) {
            // Handle any exceptions, such as database errors
            return response()->json(['success' => false, 'message' => $e->getMessage()], 500);
        }
    }
}
