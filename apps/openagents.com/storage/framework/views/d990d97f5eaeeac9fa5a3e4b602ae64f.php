/**<?php echo when(!str_contains($controller, '\\Closure'), PHP_EOL . " * @see {$controller}::" . ($isInvokable ? '__invoke' : $docblock_method ?? $method)); ?>

 * @see <?php echo $path; ?>:<?php echo $line; ?>

<?php $__currentLoopData = $parameters; $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $parameter): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
<?php if($parameter->default !== null): ?>
 * @param <?php echo $parameter->name; ?> - Default: <?php echo \Illuminate\Support\Js::from($parameter->default)->toHtml() ?>

<?php endif; ?>
<?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>
 * @route <?php echo $uri; ?>

 */
<?php /**PATH /app/vendor/laravel/wayfinder/src/../resources/docblock.blade.ts ENDPATH**/ ?>