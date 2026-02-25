import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::show
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
export const show = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(options),
    method: 'get',
})

show.definition = {
    methods: ["get","head"],
    url: '/login',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::show
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
show.url = (options?: RouteQueryOptions) => {
    return show.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::show
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
show.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::show
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
show.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: show.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::show
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
const showForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::show
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
showForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::show
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:21
* @route '/login'
*/
showForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

show.form = showForm

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
export const sendCode = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: sendCode.url(options),
    method: 'post',
})

sendCode.definition = {
    methods: ["post"],
    url: '/login/email',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
sendCode.url = (options?: RouteQueryOptions) => {
    return sendCode.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
sendCode.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: sendCode.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
const sendCodeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: sendCode.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
sendCodeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: sendCode.url(options),
    method: 'post',
})

sendCode.form = sendCodeForm

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
export const verifyCode = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: verifyCode.url(options),
    method: 'post',
})

verifyCode.definition = {
    methods: ["post"],
    url: '/login/verify',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
verifyCode.url = (options?: RouteQueryOptions) => {
    return verifyCode.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
verifyCode.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: verifyCode.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
const verifyCodeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: verifyCode.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCode
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
verifyCodeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: verifyCode.url(options),
    method: 'post',
})

verifyCode.form = verifyCodeForm

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
export const sendCodeJson = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: sendCodeJson.url(options),
    method: 'post',
})

sendCodeJson.definition = {
    methods: ["post"],
    url: '/api/auth/email',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
sendCodeJson.url = (options?: RouteQueryOptions) => {
    return sendCodeJson.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
sendCodeJson.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: sendCodeJson.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
const sendCodeJsonForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: sendCodeJson.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::sendCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
sendCodeJsonForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: sendCodeJson.url(options),
    method: 'post',
})

sendCodeJson.form = sendCodeJsonForm

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
export const verifyCodeJson = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: verifyCodeJson.url(options),
    method: 'post',
})

verifyCodeJson.definition = {
    methods: ["post"],
    url: '/api/auth/verify',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
verifyCodeJson.url = (options?: RouteQueryOptions) => {
    return verifyCodeJson.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
verifyCodeJson.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: verifyCodeJson.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
const verifyCodeJsonForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: verifyCodeJson.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verifyCodeJson
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
verifyCodeJsonForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: verifyCodeJson.url(options),
    method: 'post',
})

verifyCodeJson.form = verifyCodeJsonForm

const EmailCodeAuthController = { show, sendCode, verifyCode, sendCodeJson, verifyCodeJson }

export default EmailCodeAuthController