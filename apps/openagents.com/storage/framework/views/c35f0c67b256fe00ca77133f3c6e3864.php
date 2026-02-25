<?php if($parameters->isNotEmpty()): ?>
args<?php echo when($parameters->every->optional, '?'); ?>: {
    <?php $__currentLoopData = $parameters; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $parameter): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
        <?php echo e($parameter->name); ?><?php echo when($parameter->optional, '?'); ?>: <?php echo $parameter->types; ?>

        <?php if($parameter->key): ?>
            | { <?php echo $parameter->key; ?>: <?php echo $parameter->types; ?> }
        <?php endif; ?>,
    <?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>
}

| [
    <?php $__currentLoopData = $parameters; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $parameter): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
        <?php echo e($parameter->safeName()); ?>: <?php echo $parameter->types; ?>

        <?php if($parameter->key): ?>
            | { <?php echo $parameter->key; ?>: <?php echo $parameter->types; ?> }
         <?php endif; ?>
        <?php echo when(!$loop->last, ', '); ?>

    <?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>
]

<?php if($parameters->count() === 1): ?> | <?php echo $parameters->first()->types; ?>

    <?php if($parameters->first()->key): ?> | { <?php echo $parameters->first()->key; ?>: <?php echo $parameters->first()->types; ?> }<?php endif; ?>
<?php endif; ?>
,
<?php endif; ?>
options?: RouteQueryOptions
<?php /**PATH /app/vendor/laravel/wayfinder/src/../resources/function-arguments.blade.ts ENDPATH**/ ?>