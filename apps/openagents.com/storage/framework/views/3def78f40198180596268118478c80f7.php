<?php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
?>
# Laravel Boost Guidelines

The Laravel Boost guidelines are specifically curated by Laravel maintainers for this application. These guidelines should be followed closely to ensure the best experience when building Laravel applications.

## Foundational Context
This application is a Laravel application and its main Laravel ecosystems package & versions are below. You are an expert with them all. Ensure you abide by these specific packages & versions.

- php - <?php echo e(PHP_VERSION); ?>

<?php $__currentLoopData = app(\Laravel\Roster\Roster::class)->packages()->unique(fn ($package) => $package->rawName()); $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $package): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
- <?php echo e($package->rawName()); ?> (<?php echo e($package->name()); ?>) - v<?php echo e($package->majorVersion()); ?>

<?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>

<?php if(! empty(config('boost.purpose'))): ?>
Application purpose: <?php echo config('boost.purpose'); ?>


<?php endif; ?>

<?php if($assist->hasSkillsEnabled() && $assist->skills()->isNotEmpty()): ?>
## Skills Activation

This project has domain-specific skills available. You MUST activate the relevant skill whenever you work in that domain—don't wait until you're stuck.

<?php $__currentLoopData = $assist->skills(); $__env->addLoop($__currentLoopData); foreach($__currentLoopData as $skill): $__env->incrementLoopIndices(); $loop = $__env->getLastLoop(); ?>
- ___SINGLE_BACKTICK___<?php echo e($skill->name); ?>___SINGLE_BACKTICK___ — <?php echo e($skill->description); ?>

<?php endforeach; $__env->popLoop(); $loop = $__env->getLastLoop(); ?>
<?php endif; ?>

## Conventions
- You must follow all existing code conventions used in this application. When creating or editing a file, check sibling files for the correct structure, approach, and naming.
- Use descriptive names for variables and methods. For example, ___SINGLE_BACKTICK___isRegisteredForDiscounts___SINGLE_BACKTICK___, not ___SINGLE_BACKTICK___discount()___SINGLE_BACKTICK___.
- Check for existing components to reuse before writing a new one.

## Verification Scripts
- Do not create verification scripts or tinker when tests cover that functionality and prove they work. Unit and feature tests are more important.

## Application Structure & Architecture
- Stick to existing directory structure; don't create new base folders without approval.
- Do not change the application's dependencies without approval.

## Frontend Bundling
- If the user doesn't see a frontend change reflected in the UI, it could mean they need to run ___SINGLE_BACKTICK___<?php echo e($assist->nodePackageManagerCommand('run build')); ?>___SINGLE_BACKTICK___, ___SINGLE_BACKTICK___<?php echo e($assist->nodePackageManagerCommand('run dev')); ?>___SINGLE_BACKTICK___, or ___SINGLE_BACKTICK___<?php echo e($assist->composerCommand('run dev')); ?>___SINGLE_BACKTICK___. Ask them.

## Documentation Files
- You must only create documentation files if explicitly requested by the user.

## Replies
- Be concise in your explanations - focus on what's important rather than explaining obvious details.
<?php /**PATH /Users/christopherdavid/code/openagents/apps/openagents.com/storage/framework/views/25f8074407ac0867cde4fa5d6f6634f9.blade.php ENDPATH**/ ?>