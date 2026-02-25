import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults, validateParameters } from './../wayfinder'
/**
* @see routes/web.php:14
* @route '/'
*/
export const home = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: home.url(options),
    method: 'get',
})

home.definition = {
    methods: ["get","head"],
    url: '/',
} satisfies RouteDefinition<["get","head"]>

/**
* @see routes/web.php:14
* @route '/'
*/
home.url = (options?: RouteQueryOptions) => {
    return home.definition.url + queryParams(options)
}

/**
* @see routes/web.php:14
* @route '/'
*/
home.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: home.url(options),
    method: 'get',
})

/**
* @see routes/web.php:14
* @route '/'
*/
home.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: home.url(options),
    method: 'head',
})

/**
* @see routes/web.php:14
* @route '/'
*/
const homeForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: home.url(options),
    method: 'get',
})

/**
* @see routes/web.php:14
* @route '/'
*/
homeForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: home.url(options),
    method: 'get',
})

/**
* @see routes/web.php:14
* @route '/'
*/
homeForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: home.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

home.form = homeForm

/**
* @see \App\Http\Controllers\ChatPageController::chat
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
export const chat = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: chat.url(args, options),
    method: 'get',
})

chat.definition = {
    methods: ["get","head"],
    url: '/chat/{conversationId?}',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\ChatPageController::chat
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
chat.url = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions) => {
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

    return chat.definition.url
            .replace('{conversationId?}', parsedArgs.conversationId?.toString() ?? '')
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\ChatPageController::chat
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
chat.get = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: chat.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\ChatPageController::chat
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
chat.head = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: chat.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\ChatPageController::chat
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
const chatForm = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: chat.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\ChatPageController::chat
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
chatForm.get = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: chat.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\ChatPageController::chat
* @see app/Http/Controllers/ChatPageController.php:14
* @route '/chat/{conversationId?}'
*/
chatForm.head = (args?: { conversationId?: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: chat.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

chat.form = chatForm

/**
* @see \App\Http\Controllers\FeedPageController::feed
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
export const feed = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: feed.url(options),
    method: 'get',
})

feed.definition = {
    methods: ["get","head"],
    url: '/feed',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\FeedPageController::feed
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
feed.url = (options?: RouteQueryOptions) => {
    return feed.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\FeedPageController::feed
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
feed.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: feed.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\FeedPageController::feed
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
feed.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: feed.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\FeedPageController::feed
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
const feedForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: feed.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\FeedPageController::feed
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
feedForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: feed.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\FeedPageController::feed
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
feedForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: feed.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

feed.form = feedForm

/**
* @see routes/web.php:74
* @route '/aui'
*/
export const aui = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: aui.url(options),
    method: 'get',
})

aui.definition = {
    methods: ["get","head"],
    url: '/aui',
} satisfies RouteDefinition<["get","head"]>

/**
* @see routes/web.php:74
* @route '/aui'
*/
aui.url = (options?: RouteQueryOptions) => {
    return aui.definition.url + queryParams(options)
}

/**
* @see routes/web.php:74
* @route '/aui'
*/
aui.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: aui.url(options),
    method: 'get',
})

/**
* @see routes/web.php:74
* @route '/aui'
*/
aui.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: aui.url(options),
    method: 'head',
})

/**
* @see routes/web.php:74
* @route '/aui'
*/
const auiForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: aui.url(options),
    method: 'get',
})

/**
* @see routes/web.php:74
* @route '/aui'
*/
auiForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: aui.url(options),
    method: 'get',
})

/**
* @see routes/web.php:74
* @route '/aui'
*/
auiForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: aui.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

aui.form = auiForm

/**
* @see routes/web.php:91
* @route '/admin'
*/
export const admin = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: admin.url(options),
    method: 'get',
})

admin.definition = {
    methods: ["get","head"],
    url: '/admin',
} satisfies RouteDefinition<["get","head"]>

/**
* @see routes/web.php:91
* @route '/admin'
*/
admin.url = (options?: RouteQueryOptions) => {
    return admin.definition.url + queryParams(options)
}

/**
* @see routes/web.php:91
* @route '/admin'
*/
admin.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: admin.url(options),
    method: 'get',
})

/**
* @see routes/web.php:91
* @route '/admin'
*/
admin.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: admin.url(options),
    method: 'head',
})

/**
* @see routes/web.php:91
* @route '/admin'
*/
const adminForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: admin.url(options),
    method: 'get',
})

/**
* @see routes/web.php:91
* @route '/admin'
*/
adminForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: admin.url(options),
    method: 'get',
})

/**
* @see routes/web.php:91
* @route '/admin'
*/
adminForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: admin.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

admin.form = adminForm

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::login
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
export const login = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: login.url(options),
    method: 'get',
})

login.definition = {
    methods: ["get","head"],
    url: '/login',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::login
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
login.url = (options?: RouteQueryOptions) => {
    return login.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::login
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
login.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: login.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::login
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
login.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: login.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::login
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
const loginForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: login.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::login
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
loginForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: login.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::login
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
loginForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: login.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

login.form = loginForm

/**
* @see routes/auth.php:30
* @route '/register'
*/
export const register = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: register.url(options),
    method: 'get',
})

register.definition = {
    methods: ["get","head"],
    url: '/register',
} satisfies RouteDefinition<["get","head"]>

/**
* @see routes/auth.php:30
* @route '/register'
*/
register.url = (options?: RouteQueryOptions) => {
    return register.definition.url + queryParams(options)
}

/**
* @see routes/auth.php:30
* @route '/register'
*/
register.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: register.url(options),
    method: 'get',
})

/**
* @see routes/auth.php:30
* @route '/register'
*/
register.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: register.url(options),
    method: 'head',
})

/**
* @see routes/auth.php:30
* @route '/register'
*/
const registerForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: register.url(options),
    method: 'get',
})

/**
* @see routes/auth.php:30
* @route '/register'
*/
registerForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: register.url(options),
    method: 'get',
})

/**
* @see routes/auth.php:30
* @route '/register'
*/
registerForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: register.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

register.form = registerForm

/**
* @see routes/auth.php:40
* @route '/logout'
*/
export const logout = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: logout.url(options),
    method: 'post',
})

logout.definition = {
    methods: ["post"],
    url: '/logout',
} satisfies RouteDefinition<["post"]>

/**
* @see routes/auth.php:40
* @route '/logout'
*/
logout.url = (options?: RouteQueryOptions) => {
    return logout.definition.url + queryParams(options)
}

/**
* @see routes/auth.php:40
* @route '/logout'
*/
logout.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: logout.url(options),
    method: 'post',
})

/**
* @see routes/auth.php:40
* @route '/logout'
*/
const logoutForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: logout.url(options),
    method: 'post',
})

/**
* @see routes/auth.php:40
* @route '/logout'
*/
logoutForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: logout.url(options),
    method: 'post',
})

logout.form = logoutForm
