import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';

const env = { ...process.env };

for (const variable of ['PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH', 'FONTCONFIG_FILE']) {
  if (!env[variable]) {
    throw new Error(`${variable} must point to the Nix-provided Playwright browser/font config before running pnpm test`);
  }
  if (!existsSync(env[variable])) {
    throw new Error(`${variable} does not exist: ${env[variable]}`);
  }
}

const result = spawnSync('pnpm', ['exec', 'playwright', 'test', ...process.argv.slice(2)], {
  env,
  stdio: 'inherit',
});

if (result.error) throw result.error;
process.exit(result.status ?? 1);
