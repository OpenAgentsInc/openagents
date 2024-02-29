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
     *     summary="List all files",
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
        // ...
    }

    /**
     * @OA\Post(
     *     path="/files",
     *     tags={"File"},
     *     summary="Create a new file",
     *     operationId="createFile",
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
        // ...
    }

    /**
     * @OA\Get(
     *     path="/files/{id}",
     *     tags={"File"},
     *     summary="Get file by ID",
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
        // ...
    }

    /**
     * @OA\Put(
     *     path="/files/{id}",
     *     tags={"File"},
     *     summary="Update an existing file",
     *     operationId="updateFile",
     *
     *     @OA\Parameter(
     *         name="id",
     *         in="path",
     *         description="ID of file that needs to be updated",
     *         required=true,
     *
     *         @OA\Schema(
     *             type="integer"
     *         )
     *     ),
     *
     *     @OA\RequestBody(
     *         required=true,
     *         description="File data to update",
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
        // ...
    }

    /**
     * @OA\Delete(
     *     path="/files/{id}",
     *     tags={"File"},
     *     summary="Deletes a file",
     *     operationId="deleteFile",
     *
     *     @OA\Parameter(
     *         name="id",
     *         in="path",
     *         description="File id to delete",
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
        // ...
    }
}
