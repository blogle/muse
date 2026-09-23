import { readdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const env = { ...process.env };
const nixStore = '/nix/store';

if (existsSync(nixStore)) {
  const entries = readdirSync(nixStore);
  const browserStore = entries.find((entry) => entry.endsWith('-playwright-browsers'));
  if (browserStore) {
    const browserRoot = join(nixStore, browserStore);
    const shellDir = readdirSync(browserRoot).find((entry) => entry.startsWith('chromium_headless_shell-'));
    if (shellDir) {
      env.MUSE_PLAYWRIGHT_CHROMIUM = join(
        browserRoot,
        shellDir,
        'chrome-headless-shell-linux64',
        'chrome-headless-shell',
      );
    }
  }

  const fontsConfig = entries.find((entry) => entry.endsWith('-fonts.conf'));
  if (fontsConfig) env.FONTCONFIG_FILE ??= join(nixStore, fontsConfig);
}

const result = spawnSync('pnpm', ['exec', 'playwright', 'test', ...process.argv.slice(2)], {
  env,
  stdio: 'inherit',
});

if (result.error) throw result.error;
process.exit(result.status ?? 1);
