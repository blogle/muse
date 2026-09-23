import { defineConfig } from '@playwright/test';

declare const process: { env: Record<string, string | undefined> };

export default defineConfig({
  testDir: './tests',
  use: {
    baseURL: 'http://127.0.0.1:4173',
    browserName: 'chromium',
    launchOptions: {
      executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH,
      args: ['--no-sandbox', '--enable-unsafe-swiftshader'],
    },
  },
  webServer: { command: 'pnpm dev', url: 'http://127.0.0.1:4173', reuseExistingServer: false, timeout: 30_000 },
  reporter: 'list',
  outputDir: 'test-results/playwright',
});
