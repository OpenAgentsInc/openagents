<?php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
?>
# Laravel Pint Code Formatter

<?php if($assist->supportsPintAgentFormatter()): ?>
- You must run ___SINGLE_BACKTICK___<?php echo e($assist->binCommand('pint')); ?> --dirty --format agent___SINGLE_BACKTICK___ before finalizing changes to ensure your code matches the project's expected style.
- Do not run ___SINGLE_BACKTICK___<?php echo e($assist->binCommand('pint')); ?> --test --format agent___SINGLE_BACKTICK___, simply run ___SINGLE_BACKTICK___<?php echo e($assist->binCommand('pint')); ?> --format agent___SINGLE_BACKTICK___ to fix any formatting issues.
<?php else: ?>
- You must run ___SINGLE_BACKTICK___<?php echo e($assist->binCommand('pint')); ?> --dirty___SINGLE_BACKTICK___ before finalizing changes to ensure your code matches the project's expected style.
- Do not run ___SINGLE_BACKTICK___<?php echo e($assist->binCommand('pint')); ?> --test___SINGLE_BACKTICK___, simply run ___SINGLE_BACKTICK___<?php echo e($assist->binCommand('pint')); ?>___SINGLE_BACKTICK___ to fix any formatting issues.
<?php endif; ?>
<?php /**PATH /Users/christopherdavid/code/openagents/apps/openagents.com/storage/framework/views/369dfee60ff464becb661bd34746a79b.blade.php ENDPATH**/ ?>