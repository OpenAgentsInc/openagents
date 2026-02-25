<?php $__currentLoopData = $routes; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $route): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
    <?php echo $__env->make('wayfinder::method', [
        ...$route,
        'method' => $route['tempMethod'],
        'docblock_method' => $route['method'],
        'shouldExport' => false,
    ], array_diff_key(get_defined_vars(), ['__data' => 1, '__path' => 1]))->render(); ?>
<?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>

<?php echo when($shouldExport, 'export '); ?>const <?php echo $method; ?> = {
<?php $__currentLoopData = $routes; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $route): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
    <?php echo $route['uri']; ?>: <?php echo $route['tempMethod']; ?>,
<?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>
}<?php echo e(PHP_EOL); ?>

<?php /**PATH /Users/christopherdavid/code/openagents/apps/openagents.com/vendor/laravel/wayfinder/src/../resources/multi-method.blade.ts ENDPATH**/ ?>