/**
 * Vitest config — bundler-target browser smoke tests (T-0187).
 *
 * Runs only `test/bundler-smoke.test.ts` under `@vitest/browser` +
 * Playwright (Chromium). The default `vitest run` invocation continues
 * to use the Node pool and exclude this file (see vitest defaults +
 * the explicit include below).
 *
 * Prerequisites (handled by CI, see ts/README.md "Development"):
 *
 *     pnpm build:wasm:bundler           # produces ts/pkg/bundler/
 *     npx playwright install chromium   # downloads the browser binary
 *
 * Why Playwright over WebDriverIO: Playwright's Chromium provider is
 * the @vitest/browser default and has the most stable wasm support
 * on Linux CI runners. WebDriverIO works too but adds a Selenium hop
 * we don't need for a smoke test.
 */

import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['test/bundler-smoke.test.ts'],
    browser: {
      enabled: true,
      // `name: 'chromium'` selects the Playwright Chromium provider.
      // Firefox + WebKit are available if cross-browser parity becomes
      // a requirement; v0 sticks to Chromium for the smallest CI footprint.
      name: 'chromium',
      provider: 'playwright',
      // headless: true is the default on CI; explicit for clarity.
      headless: true,
    },
  },
});
