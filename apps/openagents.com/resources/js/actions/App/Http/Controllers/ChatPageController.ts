import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults, validateParameters } from './../../../../wayfinder'
/**
* @see \App\Http\Controllers\ChatPageController::show
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
export const show = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

show.definition = {
    methods: ["get","head"],
    url: '/chat/{conversationId?}',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\ChatPageController::show
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
show.url = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { conversationId: args }
    }

    if (Array.isArray(args)) {
        args = {
            conversationId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    validateParameters(args, [
        "conversationId",
    ])

    const parsedArgs = {
        conversationId: args?.conversationId,
    }

    return show.definition.url
            .replace('{conversationId?}', parsedArgs.conversationId?.toString() ?? '')
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\ChatPageController::show
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
show.get = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\ChatPageController::show
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
show.head = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: show.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\ChatPageController::show
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
const showForm = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\ChatPageController::show
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
showForm.get = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\ChatPageController::show
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
showForm.head = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

show.form = showForm

const ChatPageController = { show }

export default ChatPageController