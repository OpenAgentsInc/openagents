<?php
namespace App\Enums;

enum UserRole: int
{
    case USER = 0;
    case MOD = 1;
    case ADMIN = 2;
    case SUPER_ADMIN = 3;

    public static function fromInt(int $value): self
    {
        $values = self::cases();
        foreach ($values as $enum) {
            if ($enum->value === $value) {
                return $enum;
            }
        }
        return self::USER;
    }
    public function getLabel(): string
    {
        return match ($this->value) {
            self::USER->value => 'User',
            self::MOD->value => 'Moderator',
            self::ADMIN->value => 'Admin',
            self::SUPER_ADMIN->value => 'Super Admin',
            default => 'User',
        };
    }


    public function canModerate(self $other): bool
    {
        return $this->value > $other->value; // higher value means more permissions
    }

    public function getAssignableRoles(): array
    {
        $assignableRoles = [];
        $roles = self::cases();
        foreach ($roles as $role) {
            if ($this->value > $role->value) {
                $assignableRoles[] = $role;
            }
        }
        return $assignableRoles;
    }

}
