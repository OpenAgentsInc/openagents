# Feature Tests for OpenAgents User Journey

## 1. Discovery and Registration
- user can see the homepage
- user can navigate to registration page
- user can register
- user cannot register with invalid details

## 2. Account Funding (Customer)
- customer can navigate to funding page
- customer can fund their account with credit card
- customer can fund their account with bitcoin lightning
- customer receives confirmation after funding

## 3. Account Management
- user can navigate to profile
- user can edit profile
- user can update profile
- user cannot edit other users profile

## 4. Authentication
- user can login
- user cannot login with invalid credentials
- user can logout

## 5. Agent Interaction
### a. Agent Creation
- builder can create agent
- builder can edit agent
- builder cannot create agent with invalid data

### b. Agent Publishing
- builder can publish agent
- builder cannot publish agent without required fields
- user can view a published agent listing
- user cannot view an unpublished agent listing

### c. Agent Testing
- builder can test agent
- builder receives feedback from agent test

## 6. Chat Interaction
- user can start new chat with agent
- user can send message in chat
- user can receive message in chat
- user cannot send message to unauthorized agent chat

## 7. Financial Transactions (Builder)
- builder can view earnings
- builder can request withdrawal
- builder cannot withdraw with insufficient funds
- builder receives confirmation after withdrawal request

## 8. Sharing and Social Features
- user can share published agent
- user cannot share unpublished agent
- user can share earnings information
- user can invite friends to platform

## 9. Checkout and Job Configuration
- user can checkout with agent
- user can configure job after checkout
- user cannot configure job without checkout
- user receives confirmation after job configuration

## 10. Security and Permissions
- unauthenticated user cannot access protected routes
- user cannot access admin routes
- user cannot edit or delete another users agent
- user session expires after inactivity
