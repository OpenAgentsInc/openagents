# Database Models

This document describes all database models and their fields in the OpenAgents platform.

## User Model

Core user model representing registered users in the system.

Fields:
- `id` - Primary key
- `name` - User's full name
- `email` - Unique email address
- `email_verified_at` - Timestamp when email was verified (nullable)
- `current_team_id` - Foreign key to teams table, represents user's current active team (nullable)
- `current_project_id` - Foreign key to projects table, represents user's current active project (nullable)
- `password` - Hashed password
- `remember_token` - Token for "remember me" functionality
- `created_at` - Timestamp of creation
- `updated_at` - Timestamp of last update

## Team Model

Represents a team of users who can collaborate on projects.

Fields:
- `id` - Primary key
- `created_at` - Timestamp of creation
- `updated_at` - Timestamp of last update

Relations:
- Has many users through team_user pivot table

## Team User Pivot

Manages many-to-many relationship between teams and users.

Fields:
- `id` - Primary key
- `team_id` - Foreign key to teams table
- `user_id` - Foreign key to users table
- `created_at` - Timestamp of creation
- `updated_at` - Timestamp of last update

## Project Model

Represents a project within a team or owned by a user.

Fields:
- `id` - Primary key
- `name` - Project name
- `description` - Project description (nullable)
- `user_id` - Foreign key to users table, represents project owner (nullable)
- `team_id` - Foreign key to teams table, represents team that owns the project (nullable)
- `created_at` - Timestamp of creation
- `updated_at` - Timestamp of last update

## Thread Model

Represents a conversation thread within a project.

Fields:
- `id` - Primary key
- `title` - Thread title (nullable)
- `project_id` - Foreign key to projects table (nullable)
- `user_id` - Foreign key to users table, represents thread creator (nullable)
- `team_id` - Foreign key to teams table (nullable)
- `created_at` - Timestamp of creation
- `updated_at` - Timestamp of last update

## Message Model

Represents individual messages within a thread.

Fields:
- `id` - Primary key
- `user_id` - Foreign key to users table, represents message sender (nullable)
- `thread_id` - Foreign key to threads table
- `team_id` - Foreign key to teams table (nullable)
- `content` - Message content text
- `role` - Message role (user/assistant/system)
- `model` - AI model used for response (nullable)
- `input_tokens` - Number of input tokens used
- `output_tokens` - Number of output tokens used
- `created_at` - Timestamp of creation
- `updated_at` - Timestamp of last update

## File Model

Represents files associated with projects.

Fields:
- `id` - Primary key
- `name` - File name
- `path` - File path in storage
- `content` - File content (nullable)
- `project_id` - Foreign key to projects table
- `created_at` - Timestamp of creation
- `updated_at` - Timestamp of last update

## Password Reset Token Model

Manages password reset functionality.

Fields:
- `email` - Primary key, user's email address
- `token` - Reset token
- `created_at` - Timestamp of creation (nullable)

## Session Model

Manages user sessions.

Fields:
- `id` - Primary key (string)
- `user_id` - Foreign key to users table (nullable)
- `ip_address` - User's IP address (nullable)
- `user_agent` - User's browser/client info (nullable)
- `payload` - Session data
- `last_activity` - Timestamp of last activity