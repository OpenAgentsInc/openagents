import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listToolSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:12
* @route '/api/runtime/skills/tool-specs'
*/
export const listToolSpecs = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: listToolSpecs.url(options),
    method: 'get',
})

listToolSpecs.definition = {
    methods: ["get","head"],
    url: '/api/runtime/skills/tool-specs',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listToolSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:12
* @route '/api/runtime/skills/tool-specs'
*/
listToolSpecs.url = (options?: RouteQueryOptions) => {
    return listToolSpecs.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listToolSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:12
* @route '/api/runtime/skills/tool-specs'
*/
listToolSpecs.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: listToolSpecs.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listToolSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:12
* @route '/api/runtime/skills/tool-specs'
*/
listToolSpecs.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: listToolSpecs.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listToolSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:12
* @route '/api/runtime/skills/tool-specs'
*/
const listToolSpecsForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: listToolSpecs.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listToolSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:12
* @route '/api/runtime/skills/tool-specs'
*/
listToolSpecsForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: listToolSpecs.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listToolSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:12
* @route '/api/runtime/skills/tool-specs'
*/
listToolSpecsForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: listToolSpecs.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

listToolSpecs.form = listToolSpecsForm

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeToolSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:28
* @route '/api/runtime/skills/tool-specs'
*/
export const storeToolSpec = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: storeToolSpec.url(options),
    method: 'post',
})

storeToolSpec.definition = {
    methods: ["post"],
    url: '/api/runtime/skills/tool-specs',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeToolSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:28
* @route '/api/runtime/skills/tool-specs'
*/
storeToolSpec.url = (options?: RouteQueryOptions) => {
    return storeToolSpec.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeToolSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:28
* @route '/api/runtime/skills/tool-specs'
*/
storeToolSpec.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: storeToolSpec.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeToolSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:28
* @route '/api/runtime/skills/tool-specs'
*/
const storeToolSpecForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: storeToolSpec.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeToolSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:28
* @route '/api/runtime/skills/tool-specs'
*/
storeToolSpecForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: storeToolSpec.url(options),
    method: 'post',
})

storeToolSpec.form = storeToolSpecForm

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listSkillSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:49
* @route '/api/runtime/skills/skill-specs'
*/
export const listSkillSpecs = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: listSkillSpecs.url(options),
    method: 'get',
})

listSkillSpecs.definition = {
    methods: ["get","head"],
    url: '/api/runtime/skills/skill-specs',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listSkillSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:49
* @route '/api/runtime/skills/skill-specs'
*/
listSkillSpecs.url = (options?: RouteQueryOptions) => {
    return listSkillSpecs.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listSkillSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:49
* @route '/api/runtime/skills/skill-specs'
*/
listSkillSpecs.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: listSkillSpecs.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listSkillSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:49
* @route '/api/runtime/skills/skill-specs'
*/
listSkillSpecs.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: listSkillSpecs.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listSkillSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:49
* @route '/api/runtime/skills/skill-specs'
*/
const listSkillSpecsForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: listSkillSpecs.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listSkillSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:49
* @route '/api/runtime/skills/skill-specs'
*/
listSkillSpecsForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: listSkillSpecs.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::listSkillSpecs
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:49
* @route '/api/runtime/skills/skill-specs'
*/
listSkillSpecsForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: listSkillSpecs.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

listSkillSpecs.form = listSkillSpecsForm

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:65
* @route '/api/runtime/skills/skill-specs'
*/
export const storeSkillSpec = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: storeSkillSpec.url(options),
    method: 'post',
})

storeSkillSpec.definition = {
    methods: ["post"],
    url: '/api/runtime/skills/skill-specs',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:65
* @route '/api/runtime/skills/skill-specs'
*/
storeSkillSpec.url = (options?: RouteQueryOptions) => {
    return storeSkillSpec.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:65
* @route '/api/runtime/skills/skill-specs'
*/
storeSkillSpec.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: storeSkillSpec.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:65
* @route '/api/runtime/skills/skill-specs'
*/
const storeSkillSpecForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: storeSkillSpec.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::storeSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:65
* @route '/api/runtime/skills/skill-specs'
*/
storeSkillSpecForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: storeSkillSpec.url(options),
    method: 'post',
})

storeSkillSpec.form = storeSkillSpecForm

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::publishSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:86
* @route '/api/runtime/skills/skill-specs/{skillId}/{version}/publish'
*/
export const publishSkillSpec = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: publishSkillSpec.url(args, options),
    method: 'post',
})

publishSkillSpec.definition = {
    methods: ["post"],
    url: '/api/runtime/skills/skill-specs/{skillId}/{version}/publish',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::publishSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:86
* @route '/api/runtime/skills/skill-specs/{skillId}/{version}/publish'
*/
publishSkillSpec.url = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions) => {
    if (Array.isArray(args)) {
        args = {
            skillId: args[0],
            version: args[1],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        skillId: args.skillId,
        version: args.version,
    }

    return publishSkillSpec.definition.url
            .replace('{skillId}', parsedArgs.skillId.toString())
            .replace('{version}', parsedArgs.version.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::publishSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:86
* @route '/api/runtime/skills/skill-specs/{skillId}/{version}/publish'
*/
publishSkillSpec.post = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: publishSkillSpec.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::publishSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:86
* @route '/api/runtime/skills/skill-specs/{skillId}/{version}/publish'
*/
const publishSkillSpecForm = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: publishSkillSpec.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::publishSkillSpec
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:86
* @route '/api/runtime/skills/skill-specs/{skillId}/{version}/publish'
*/
publishSkillSpecForm.post = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: publishSkillSpec.url(args, options),
    method: 'post',
})

publishSkillSpec.form = publishSkillSpecForm

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::showSkillRelease
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:103
* @route '/api/runtime/skills/releases/{skillId}/{version}'
*/
export const showSkillRelease = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: showSkillRelease.url(args, options),
    method: 'get',
})

showSkillRelease.definition = {
    methods: ["get","head"],
    url: '/api/runtime/skills/releases/{skillId}/{version}',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::showSkillRelease
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:103
* @route '/api/runtime/skills/releases/{skillId}/{version}'
*/
showSkillRelease.url = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions) => {
    if (Array.isArray(args)) {
        args = {
            skillId: args[0],
            version: args[1],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        skillId: args.skillId,
        version: args.version,
    }

    return showSkillRelease.definition.url
            .replace('{skillId}', parsedArgs.skillId.toString())
            .replace('{version}', parsedArgs.version.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::showSkillRelease
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:103
* @route '/api/runtime/skills/releases/{skillId}/{version}'
*/
showSkillRelease.get = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: showSkillRelease.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::showSkillRelease
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:103
* @route '/api/runtime/skills/releases/{skillId}/{version}'
*/
showSkillRelease.head = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: showSkillRelease.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::showSkillRelease
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:103
* @route '/api/runtime/skills/releases/{skillId}/{version}'
*/
const showSkillReleaseForm = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: showSkillRelease.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::showSkillRelease
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:103
* @route '/api/runtime/skills/releases/{skillId}/{version}'
*/
showSkillReleaseForm.get = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: showSkillRelease.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeSkillRegistryController::showSkillRelease
* @see app/Http/Controllers/Api/RuntimeSkillRegistryController.php:103
* @route '/api/runtime/skills/releases/{skillId}/{version}'
*/
showSkillReleaseForm.head = (args: { skillId: string | number, version: string | number } | [skillId: string | number, version: string | number ], options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: showSkillRelease.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

showSkillRelease.form = showSkillReleaseForm

const RuntimeSkillRegistryController = { listToolSpecs, storeToolSpec, listSkillSpecs, storeSkillSpec, publishSkillSpec, showSkillRelease }

export default RuntimeSkillRegistryController