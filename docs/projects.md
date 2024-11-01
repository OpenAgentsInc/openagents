# Projects in OpenAgents

OpenAgents implements a project-based organization system that allows users to group related threads and resources together, enabling more effective collaboration and knowledge management. This document outlines how projects are implemented and used throughout the platform.

## Core Concepts

### Project Organization
- Projects can belong to either a user (personal) or a team
- Projects serve as containers for related threads and resources
- Projects can have custom instructions and context
- Projects support a 200K context window for documents and knowledge
- Projects require unique names within their team/user scope
- Projects can be archived when no longer actively needed

### Project Resources
Projects serve as organizational units that contain:
- Chat threads
- Documents and files
- Custom instructions
- Shared context
- Knowledge base
- Custom settings (e.g., tone, language, role)

## Implementation Details

### Models and Relationships

#### Project Model
- Can belong to either a user or a team (polymorphic)
- Has many threads
- Has many files/documents
- Has custom instructions
- Has project-specific context settings
- Supports custom settings as JSON data
- Enforces name uniqueness within scope

#### Thread Model
- Belongs to a project
- Has many messages
- Inherits project context and instructions
- Can be shared within team (if team project)
- Returns empty context/instructions when no project is associated

### Project Features

#### Context Management
- Projects maintain a 200K context window
- Support for document ingestion and reference
- Custom instructions per project
- Persistent context across threads
- Configurable project-wide settings (tone, language, role)

#### Knowledge Integration
- Document upload and processing
- Code repository integration
- Style guide incorporation
- Interview/meeting transcript storage
- Past work reference
- File management with content ingestion

#### Collaboration
- Team members can view shared projects
- Thread sharing within projects
- Knowledge base sharing
- Context inheritance across team
- Strict access control for team projects

## Frontend Implementation

The project interface provides:
- Project creation and management
- Document upload and organization
- Thread organization and viewing
- Context and instruction management
- Team collaboration tools

## Testing

The project functionality is extensively tested with both unit and feature tests:

### Verified Unit Tests
✓ Project ownership and relationships
- Projects can belong to either users or teams
- Projects have many threads
- Projects have many files
- Projects belong to either user or team exclusively

✓ Access Control
- Team members can access team projects
- Non-team members cannot access team projects
- Project access follows team membership rules

✓ Data Validation
- Projects require a name
- Project names must be unique within team/user scope
- Projects support custom instructions
- Projects can have custom settings (JSON)

✓ Thread Integration
- Threads inherit project context
- Threads inherit project instructions
- Threads properly handle missing project context
- Default project behavior for team threads

✓ Project Management
- Projects can be archived
- Projects maintain proper ownership scope
- Projects support file associations

### Key Test Cases
- Project creation and ownership
- Thread organization
- Document management
- Context inheritance
- Team access controls
- File upload and processing
- Settings management

## Security Considerations

- Projects are scoped to users or teams
- Resource access follows project ownership
- Team projects respect team membership
- Personal projects remain private
- Document access is controlled
- Validation ensures data integrity

## Future Enhancements

- Project templates
- Advanced document processing
- Enhanced collaboration features
- Project analytics
- Cross-project knowledge sharing
- Project archival and backup
- Export/import capabilities
- Improved file processing system

## Usage Guidelines

### Creating Projects
1. Choose personal or team context
2. Set project name and description (must be unique)
3. Configure custom instructions
4. Add initial documents/context
5. Create or import threads
6. Configure project settings

### Managing Context
- Upload relevant documents
- Set custom instructions
- Define project-specific parameters
- Maintain knowledge base
- Configure project settings (tone, language, role)

### Collaboration
- Share projects within teams
- Organize threads by topic
- Maintain shared context
- Leverage team knowledge
- Respect access control boundaries