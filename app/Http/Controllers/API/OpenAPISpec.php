<?php

namespace App\Http\Controllers\API;

/**
 * @OA\Info(
 *     version="1.0",
 *     title="OpenAgents API",
 *     description="The OpenAgents API",
 *
 *     @OA\Contact(
 *         name="OpenAgents",
 *         url="https://x.com/OpenAgentsInc"
 *     )
 * )
 *
 * @OA\Server(
 *     url="https://openagents.com/api/v1",
 *     description="OpenAgents API server"
 * )
 *
 * @OA\Components(
 *
 *      @OA\Schema(
 *          schema="File",
 *          type="object",
 *
 *          @OA\Property(
 *              property="id",
 *              type="integer",
 *              format="int64",
 *              description="File ID"
 *          ),
 *          @OA\Property(
 *              property="path",
 *              type="string",
 *              description="File path"
 *          ),
 *          @OA\Property(
 *              property="description",
 *              type="string",
 *              description="File description"
 *          ),
 *          @OA\Property(
 *              property="created_at",
 *              type="string",
 *              format="date-time",
 *              description="File creation date and time"
 *          ),
 *          @OA\Property(
 *              property="updated_at",
 *              type="string",
 *              format="date-time",
 *              description="File update date and time"
 *          ),
 *          @OA\Property(
 *              property="user_id",
 *              type="integer",
 *              format="int64",
 *              description="User ID"
 *          ),
 *          @OA\Property(
 *              property="agent_id",
 *              type="integer",
 *              format="int64",
 *              description="Agent ID"
 *          )
 *      ),
 *
 *     @OA\Schema(
 *         schema="Agent",
 *         type="object",
 *
 *         @OA\Property(
 *             property="id",
 *             type="integer",
 *             format="int64",
 *             description="Agent ID"
 *         ),
 *         @OA\Property(
 *             property="name",
 *             type="string",
 *             description="Agent name"
 *         ),
 *         @OA\Property(
 *             property="description",
 *             type="string",
 *             description="Agent description"
 *         ),
 *         @OA\Property(
 *             property="instructions",
 *             type="string",
 *             description="Agent instructions"
 *         ),
 *         @OA\Property(
 *             property="welcome_message",
 *             type="string",
 *             description="Agent welcome message"
 *         )
 *     ),
 *
 *     @OA\Schema(
 *         schema="AgentList",
 *         type="array",
 *
 *         @OA\Items(ref="#/components/schemas/Agent")
 *     )
 * )
 */
class OpenApiSpec
{
}
