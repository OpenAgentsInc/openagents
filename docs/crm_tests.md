# CRM Tests

This document outlines all required tests for the CRM functionality in OpenAgents. Tests are organized by type (Feature/Unit) and file.

## Feature Tests

### ContactTest.php
```
it can create a new contact
it can list contacts for a team
it can search contacts by name or email
it can filter contacts by tags
it can update contact information
it can delete a contact
it enforces contact permissions by team
it can merge duplicate contacts
it can import contacts from CSV
it tracks contact view history
```

### ContactActivityTest.php
```
it logs email interactions with contacts
it logs chat interactions with contacts
it logs meeting interactions with contacts
it can view activity timeline for a contact
it associates AI chat threads with contacts
it tracks last contact date automatically
it can add manual activity notes
it can edit activity entries
it can delete activity entries
```

### ContactAITest.php
```
it generates contact insights from chat history
it suggests follow-up actions for contacts
it calculates engagement scores
it identifies relationship risks
it summarizes contact interaction history
it generates meeting preparation notes
it suggests personalized talking points
```

### ContactTeamTest.php
```
it assigns contacts to teams
it shares contacts between teams
it transfers contact ownership
it manages contact access permissions
it notifies team on contact updates
```

### ContactEmailTest.php
```
it syncs emails with contacts
it creates contacts from emails
it tracks email open rates
it manages email templates
it schedules follow-up emails
it logs bounced emails
```

### ContactSearchTest.php
```
it performs full-text search on contacts
it filters contacts by multiple criteria
it sorts contacts by various fields
it paginates contact results
it exports contact search results
```

## Unit Tests

### Models/ContactTest.php
```
it belongs to a team
it has many activities
it has many email threads
it has many chat threads
it has many notes
it has many tags
it calculates engagement score
it validates required fields
it formats phone numbers
it generates unique contact IDs
```

### Models/ContactActivityTest.php
```
it belongs to a contact
it belongs to a user
it has activity type
it has timestamp
it has optional notes
it links to related content
it validates activity data
```

### Models/ContactTagTest.php
```
it belongs to a contact
it belongs to a team
it has unique constraints
it validates tag format
```

### Models/ContactEmailTest.php
```
it belongs to a contact
it tracks email metadata
it handles email threading
it manages email attachments
it validates email addresses
```

### Models/ContactNoteTest.php
```
it belongs to a contact
it belongs to a user
it supports markdown formatting
it tracks edit history
it handles mentions
```

### Services/ContactMergeServiceTest.php
```
it merges contact basic info
it merges contact activities
it merges contact emails
it merges contact notes
it handles conflict resolution
it maintains audit trail
```

### Services/ContactImportServiceTest.php
```
it validates import format
it maps import fields
it handles duplicate detection
it processes batch imports
it reports import errors
```

### Services/ContactAIServiceTest.php
```
it analyzes contact interactions
it generates contact summaries
it calculates relationship scores
it identifies action items
it suggests follow-ups
```

### Services/ContactSearchServiceTest.php
```
it indexes contact data
it performs fuzzy matching
it ranks search results
it filters by permissions
it optimizes query performance
```

## Integration Tests

### CRM/ContactEmailIntegrationTest.php
```
it syncs with Gmail API
it syncs with Outlook API
it handles email threading
it manages email attachments
it tracks email status
```

### CRM/ContactCalendarIntegrationTest.php
```
it syncs calendar events
it creates meeting records
it sends meeting reminders
it updates meeting status
```

### CRM/ContactAIIntegrationTest.php
```
it integrates with chat history
it processes email content
it analyzes meeting transcripts
it generates insights
```

## Performance Tests

### CRM/ContactPerformanceTest.php
```
it handles large contact lists
it performs bulk operations efficiently
it maintains search performance
it optimizes database queries
it manages memory usage
```

## Security Tests

### CRM/ContactSecurityTest.php
```
it enforces team permissions
it validates data access
it logs security events
it handles sensitive data
it manages API access
```

## Notes

1. All tests should follow our existing naming conventions
2. Feature tests focus on user-facing functionality
3. Unit tests ensure data model integrity
4. Integration tests verify external service connections
5. Performance tests ensure scalability
6. Security tests protect data access

## Test Implementation Priority

1. Core Model Tests (Unit)
2. Basic CRUD Operations (Feature)
3. Team Permission Tests (Security)
4. Activity Logging (Feature)
5. Search Functionality (Feature)
6. AI Integration (Integration)
7. Email Integration (Integration)
8. Performance Optimization (Performance)

## Test Data Requirements

1. Sample contacts dataset
2. Email interaction history
3. Chat thread examples
4. Calendar entries
5. Team structures
6. Permission matrices