# Manual Testing Checklist - MVP Launch Readiness
**Date**: June 28, 2025  
**Tester**: Claude  
**Environment**: Development (localhost:3000)

## Pre-Test Setup
- [ ] Ensure development server is running (`pnpm dev`)
- [ ] Clear browser cache and localStorage
- [ ] Open browser developer console for error monitoring
- [ ] Prepare test accounts (GitHub OAuth)

## Test Scenarios

### 1. First-Time User Experience (Not Authenticated)
- [ ] **Homepage Load**
  - [ ] Page loads without errors
  - [ ] Claude Artifacts-style interface displays correctly
  - [ ] "Log in with GitHub" button visible in top-right
  - [ ] Chat interface displays on left
  - [ ] Empty artifacts panel on right
  - [ ] No console errors

- [ ] **Desktop-Only Enforcement**
  - [ ] Resize browser below 1024px width
  - [ ] Desktop required overlay should appear
  - [ ] Resize back above 1024px
  - [ ] Overlay should disappear

### 2. Authentication Flow
- [ ] **GitHub OAuth Login**
  - [ ] Click "Log in with GitHub" button
  - [ ] Redirected to GitHub OAuth page
  - [ ] Authorize app (if first time)
  - [ ] Redirected back to app
  - [ ] User authenticated successfully
  - [ ] GitHub button disappears
  - [ ] User menu/profile appears

### 3. Chat to Code Generation Flow
- [ ] **Basic Code Generation**
  - [ ] Type "Build a Bitcoin price tracker app" in chat
  - [ ] Send message
  - [ ] AI response starts streaming
  - [ ] Artifact automatically created in right panel
  - [ ] Code displays with syntax highlighting
  - [ ] Artifact has title, navigation controls, action buttons

- [ ] **Multiple Artifacts**
  - [ ] Send another message: "Create a dashboard with charts"
  - [ ] New artifact created
  - [ ] Navigation shows "2 of 2"
  - [ ] Can navigate between artifacts with arrow buttons
  - [ ] Previous artifact still accessible

### 4. Artifact Management
- [ ] **Code/Preview Toggle**
  - [ ] Click "Preview" button
  - [ ] Preview panel shows (with deploy prompt if not deployed)
  - [ ] Click "Code" button
  - [ ] Code editor returns

- [ ] **Copy Functionality**
  - [ ] Click copy button
  - [ ] Toast notification appears
  - [ ] Paste in external editor - code should paste correctly

- [ ] **Download Functionality**
  - [ ] Click download button
  - [ ] File downloads as .tsx
  - [ ] File contains correct code

- [ ] **Deploy Functionality**
  - [ ] Click "Deploy" button
  - [ ] Deployment process starts
  - [ ] Button shows loading state
  - [ ] Deployment completes (~2 seconds)
  - [ ] Deploy button changes to external link icon
  - [ ] Preview mode now shows live iframe

### 5. Error Scenarios
- [ ] **Network Interruption**
  - [ ] Disable network in DevTools
  - [ ] Try to send chat message
  - [ ] Error handling should be graceful
  - [ ] Re-enable network
  - [ ] App should recover

- [ ] **Invalid Input**
  - [ ] Send empty message
  - [ ] Should be prevented or handled gracefully
  - [ ] Send extremely long message (1000+ chars)
  - [ ] Should handle without breaking UI

### 6. Performance Testing
- [ ] **Response Times**
  - [ ] Chat message response: < 1 second to start streaming
  - [ ] Artifact creation: Immediate
  - [ ] Code/Preview toggle: < 100ms
  - [ ] Navigation between artifacts: < 100ms

- [ ] **UI Responsiveness**
  - [ ] Type rapidly in chat input
  - [ ] No lag or dropped characters
  - [ ] Scroll through long code
  - [ ] Smooth scrolling, no jank

### 7. State Persistence
- [ ] **LocalStorage Persistence**
  - [ ] Create 2-3 artifacts
  - [ ] Refresh page
  - [ ] Artifacts should persist
  - [ ] Current artifact selection maintained

- [ ] **Session Recovery**
  - [ ] Open app in new tab
  - [ ] Should maintain authentication
  - [ ] Artifacts accessible

### 8. Cross-Browser Testing
- [ ] **Chrome** - All features work
- [ ] **Firefox** - All features work
- [ ] **Safari** - All features work
- [ ] **Edge** - All features work

## Test Results Summary

### ✅ Passed Tests
- List all passed test cases

### ❌ Failed Tests
- List all failed test cases with details

### ⚠️ Issues Found
- List any bugs, UX issues, or concerns

### 📊 Performance Metrics
- Page load time: 
- Time to interactive:
- Chat response time:
- Deployment time:

### 🎯 Overall Assessment
- [ ] Ready for launch
- [ ] Needs fixes before launch
- [ ] Major blockers found

## Recommendations
- Priority fixes needed before launch
- Nice-to-have improvements
- Post-launch monitoring suggestions