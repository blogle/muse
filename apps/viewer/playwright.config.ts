import { defineConfig } from '@playwright/test';

declare const process: { env: Record<string, string | undefined> };

export default defineConfig({
  testDir: './tests',
  use: {
    baseURL: 'http://127.0.0.1:4173',
    browserName: 'chromium',
    launchOptions: {
      executablePath: process.env.MUSE_PLAYWRIGHT_CHROMIUM,
      args: ['--no-sandbox', '--enable-unsafe-swiftshader'],
    },
  },
  webServer: { command: 'pnpm dev', url: 'http://127.0.0.1:4173', reuseExistingServer: false, timeout: 30_000 },
  reporter: 'list',
  outputDir: 'test-results/playwright',
});
