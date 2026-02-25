<?php echo $__env->make('wayfinder::docblock', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>
<?php echo when($shouldExport, 'export '); ?>const <?php echo $method; ?> = (<?php echo $__env->make('wayfinder::function-arguments', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>): RouteDefinition<<?php echo \Illuminate\Support\Js::from($verbs->first()->actual)->toHtml() ?>> => ({
    url: <?php echo $method; ?>.url(<?php echo when($parameters->isNotEmpty(), 'args, '); ?>options),
    method: <?php echo \Illuminate\Support\Js::from($verbs->first()->actual)->toHtml() ?>,
})

<?php echo $method; ?>.definition = {
    methods: <?php echo $verbs->pluck('actual')->toJson(); ?>,
    url: <?php echo $uri; ?>,
} satisfies RouteDefinition<<?php echo $verbs->pluck('actual')->toJson(); ?>>

<?php echo $__env->make('wayfinder::docblock', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>
<?php echo $method; ?>.url = (<?php echo $__env->make('wayfinder::function-arguments', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>) => {
<?php if($parameters->count() === 1): ?>
    if (typeof args === 'string' || typeof args === 'number') {
        args = { <?php echo $parameters->first()->name; ?>: args }
    }

    <?php if($parameters->first()->key): ?>
        if (typeof args === 'object' && !Array.isArray(args) && <?php echo \Illuminate\Support\Js::from($parameters->first()->key)->toHtml() ?> in args) {
            args = { <?php echo $parameters->first()->name; ?>: args.<?php echo $parameters->first()->key; ?> }
        }
    <?php endif; ?>
<?php endif; ?>

<?php if($parameters->isNotEmpty()): ?>
    if (Array.isArray(args)) {
        args = {
        <?php $__currentLoopData = $parameters; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $parameter): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
            <?php echo $parameter->name; ?>: args[<?php echo $loop->index; ?>],
        <?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>
        }
    }

    args = applyUrlDefaults(args)
<?php endif; ?>

<?php if($parameters->where('optional')->isNotEmpty()): ?>
    validateParameters(args, [
    <?php $__currentLoopData = $parameters->where('optional'); $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $parameter): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
        "<?php echo $parameter->name; ?>",
    <?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>
    ])
<?php endif; ?>

<?php if($parameters->isNotEmpty()): ?>
    const parsedArgs = {
    <?php $__currentLoopData = $parameters; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $parameter): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
        <?php if($parameter->key): ?>
            <?php echo $parameter->name; ?>: <?php echo when($parameter->default !== null, '('); ?>typeof args<?php echo when($parameters->every->optional, '?'); ?>.<?php echo $parameter->name; ?> === 'object'
                ? args.<?php echo $parameter->name; ?>.<?php echo $parameter->key ?? 'id'; ?>

                : args<?php echo when($parameters->every->optional, '?'); ?>.<?php echo $parameter->name; ?><?php echo when($parameter->default !== null, ') ?? '); ?><?php if($parameter->default !== null): ?><?php echo \Illuminate\Support\Js::from($parameter->default)->toHtml() ?><?php endif; ?>,
        <?php else: ?>
            <?php echo $parameter->name; ?>: args<?php echo when($parameters->every->optional, '?'); ?>.<?php echo $parameter->name; ?><?php echo when($parameter->default !== null, ' ?? '); ?><?php if($parameter->default !== null): ?><?php echo \Illuminate\Support\Js::from($parameter->default)->toHtml() ?><?php endif; ?>,
        <?php endif; ?>
    <?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>
    }
<?php endif; ?>

    return <?php echo $method; ?>.definition.url
<?php $__currentLoopData = $parameters; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $parameter): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
            .replace(<?php echo \Illuminate\Support\Js::from($parameter->placeholder)->toHtml() ?>, parsedArgs.<?php echo $parameter->name; ?><?php echo when($parameter->optional, '?'); ?>.toString()<?php echo when($parameter->optional, " ?? ''"); ?>)
    <?php if($loop->last): ?>
            .replace(/\/+$/, '')
    <?php endif; ?>
<?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?> + queryParams(options)
}

<?php $__currentLoopData = $verbs; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $verb): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
<?php echo $__env->make('wayfinder::docblock', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>
<?php echo $method; ?>.<?php echo $verb->actual; ?> = (<?php echo $__env->make('wayfinder::function-arguments', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>): RouteDefinition<<?php echo \Illuminate\Support\Js::from($verb->actual)->toHtml() ?>> => ({
    url: <?php echo $method; ?>.url(<?php echo when($parameters->isNotEmpty(), 'args, '); ?>options),
    method: <?php echo \Illuminate\Support\Js::from($verb->actual)->toHtml() ?>,
})
<?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>

<?php if($withForm): ?>
    <?php echo $__env->make('wayfinder::docblock', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>
    const <?php echo $method; ?>Form = (<?php echo $__env->make('wayfinder::function-arguments', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>): RouteFormDefinition<<?php echo \Illuminate\Support\Js::from($verbs->first()->formSafe)->toHtml() ?>> => ({
        action: <?php echo $method; ?>.url(
            <?php echo when($parameters->isNotEmpty(), 'args, '); ?>

            <?php if($verbs->first()->formSafe === $verbs->first()->actual): ?>
                options
            <?php else: ?>
                {
                    [options?.mergeQuery ? 'mergeQuery' : 'query']: {
                        _method: <?php echo \Illuminate\Support\Js::from(strtoupper($verbs->first()->actual))->toHtml() ?>,
                        ...(options?.query ?? options?.mergeQuery ?? {}),
                    }
                }
            <?php endif; ?>
        ),
        method: <?php echo \Illuminate\Support\Js::from($verbs->first()->formSafe)->toHtml() ?>,
    })

    <?php $__currentLoopData = $verbs; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $verb): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
        <?php echo $__env->make('wayfinder::docblock', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>
        <?php echo $method; ?>Form.<?php echo $verb->actual; ?> = (<?php echo $__env->make('wayfinder::function-arguments', array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>): RouteFormDefinition<<?php echo \Illuminate\Support\Js::from($verb->formSafe)->toHtml() ?>> => ({
            action: <?php echo $method; ?>.url(
                <?php echo when($parameters->isNotEmpty(), 'args, '); ?>

                <?php if($verb->formSafe === $verb->actual): ?>
                options
                <?php else: ?>
                    {
                        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
                            _method: <?php echo \Illuminate\Support\Js::from(strtoupper($verb->actual))->toHtml() ?>,
                            ...(options?.query ?? options?.mergeQuery ?? {}),
                        }
                    }
                <?php endif; ?>
            ),
            method: <?php echo \Illuminate\Support\Js::from($verb->formSafe)->toHtml() ?>,
        })
    <?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>

    <?php echo $method; ?>.form = <?php echo $method; ?>Form
<?php endif; ?>
<?php /**PATH /app/vendor/laravel/wayfinder/src/../resources/method.blade.ts ENDPATH**/ ?>