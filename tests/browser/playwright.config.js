import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: '*.spec.js',
  workers: 1,
  webServer: {
    command: 'cargo run --locked --manifest-path ../../Cargo.toml -p kanlo-api --bin kanlo-server',
    url: 'http://127.0.0.1:18080/api/v1/health',
    reuseExistingServer: !process.env.CI,
    env: { KANLO_BIND: '127.0.0.1:18080', KANLO_DATA_DIR: '../../data/browser-tests' },
    timeout: 120000,
  },
  use: { baseURL: 'http://127.0.0.1:18080' },
  projects: [
    { name: 'desktop', use: { viewport: { width: 1440, height: 1000 } } },
    { name: 'mobile', use: { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true } },
  ],
});
