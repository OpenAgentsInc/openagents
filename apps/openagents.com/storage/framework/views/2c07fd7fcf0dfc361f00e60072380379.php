<?php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
?>
# Test Enforcement

- Every change must be programmatically tested. Write a new test or update an existing test, then run the affected tests to make sure they pass.
- Run the minimum number of tests needed to ensure code quality and speed. Use ___SINGLE_BACKTICK___<?php echo e($assist->artisanCommand('test --compact')); ?>___SINGLE_BACKTICK___ with a specific filename or filter.
<?php /**PATH /Users/christopherdavid/code/openagents/apps/openagents.com/storage/framework/views/c171fffd32ee4668d0f75682681d7fac.blade.php ENDPATH**/ ?>