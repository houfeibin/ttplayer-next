import { defineConfig } from '@playwright/test';

/**
 * Playwright config for TTPlayer-Next (Tauri 2.0 E2E tests)
 *
 * Tauri uses a custom protocol (tauri://) so we need special launch args.
 * Tests run against the built Tauri app via WebDriver or direct CDP.
 *
 * Usage:
 *   npx playwright test           — run all tests
 *   npx playwright test --ui      — interactive UI mode
 *   npx playwright show-report    — view report
 */
export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  retries: 0,
  workers: 1, // Tauri app is single-instance

  reporter: [
    ['html', { open: 'never' }],
    ['list'],
  ],

  use: {
    // Tauri app URL scheme
    baseURL: 'tauri://localhost',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: [
    {
      name: 'tauri-chromium',
      use: {
        browserName: 'chromium',
        launchOptions: {
          args: [
            '--disable-gpu',
            '--no-sandbox',
          ],
        },
      },
    },
  ],
});
