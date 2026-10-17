-- Deletion uses the same accepted-operation recovery as suspension and wake.
ALTER TABLE computers.lifecycle_operations
    DROP CONSTRAINT lifecycle_operations_action_check;
ALTER TABLE computers.lifecycle_operations
    ADD CONSTRAINT lifecycle_operations_action_check
    CHECK (action IN ('suspend', 'wake', 'delete'));
