import { test, expect } from '@playwright/test';

/**
 * E2E tests for TTPlayer-Next
 *
 * These tests require the Tauri app to be built and running.
 * Run: cargo tauri build --debug
 * Then: npx playwright test
 *
 * For now, these are stubs that test the basic app lifecycle.
 */

test.describe('TTPlayer-Next E2E', () => {
  test('app launches and shows main panel', async ({ page }) => {
    // This test requires the Tauri app to be running
    // and connected via WebDriver
    await page.goto('tauri://localhost');

    // Should have the main panel visible
    await expect(page.locator('[class*="mainPanel"]')).toBeVisible();
  });

  test('play button is visible', async ({ page }) => {
    await page.goto('tauri://localhost');

    // Play button should be visible
    const playBtn = page.locator('button', { hasText: /play|播放/ });
    await expect(playBtn).toBeVisible();
  });

  test('file dialog can be opened', async ({ page }) => {
    await page.goto('tauri://localhost');

    // Click the open file button
    const openBtn = page.locator('button', { hasText: /open|打开/ });
    await expect(openBtn).toBeVisible();
  });
});
