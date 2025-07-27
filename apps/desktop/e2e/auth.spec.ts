import { test, expect } from '@playwright/test';

test.describe('Authentication Flow', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the app
    await page.goto('/');
    // Wait for app to load
    await page.waitForLoadState('networkidle');
  });

  test('should display login button when unauthenticated', async ({ page }) => {
    // Look for login button - this will depend on the actual UI structure
    const loginButton = page.getByRole('button', { name: /login/i });
    await expect(loginButton).toBeVisible();
  });

  test('should show loading state during authentication', async ({ page }) => {
    // Click login button
    const loginButton = page.getByRole('button', { name: /login/i });
    await loginButton.click();

    // Check for loading state
    const loadingIndicator = page.getByText(/loading/i);
    await expect(loadingIndicator).toBeVisible();
  });

  test('should handle authentication error gracefully', async ({ page }) => {
    // Mock network request to fail
    await page.route('**/token', route => {
      route.fulfill({
        status: 400,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'Invalid request' })
      });
    });

    // Attempt login
    const loginButton = page.getByRole('button', { name: /login/i });
    await loginButton.click();

    // Wait for error message (this depends on how errors are displayed)
    await expect(page.getByText(/error/i)).toBeVisible();
  });

  test('should persist authentication state on page reload', async ({ page }) => {
    // This test would require mocking successful authentication
    // and checking that the state persists after page reload
    
    // Mock successful auth response
    await page.route('**/token', route => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          access_token: 'mock-token',
          user: {
            id: 'test-user',
            githubUsername: 'testuser',
            email: 'test@example.com',
            githubId: 'github-123'
          }
        })
      });
    });

    // Simulate login flow
    const loginButton = page.getByRole('button', { name: /login/i });
    if (await loginButton.isVisible()) {
      await loginButton.click();
      
      // Wait for authentication to complete
      await expect(page.getByText(/testuser/i)).toBeVisible();
      
      // Reload page
      await page.reload();
      await page.waitForLoadState('networkidle');
      
      // Check that user is still authenticated
      await expect(page.getByText(/testuser/i)).toBeVisible();
    }
  });

  test('should allow logout and return to unauthenticated state', async ({ page }) => {
    // Mock successful auth response first
    await page.route('**/token', route => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          access_token: 'mock-token',
          user: {
            id: 'test-user',
            githubUsername: 'testuser',
            email: 'test@example.com',
            githubId: 'github-123'
          }
        })
      });
    });

    // Login first
    const loginButton = page.getByRole('button', { name: /login/i });
    if (await loginButton.isVisible()) {
      await loginButton.click();
      await expect(page.getByText(/testuser/i)).toBeVisible();
      
      // Find and click logout button
      const logoutButton = page.getByRole('button', { name: /logout/i });
      await logoutButton.click();
      
      // Verify return to unauthenticated state
      await expect(page.getByRole('button', { name: /login/i })).toBeVisible();
    }
  });
});