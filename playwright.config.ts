import { defineConfig, devices } from '@playwright/test';

const everyone = (origin: string) => ({
  cookies: [],
  origins: [
    {
      origin,
      localStorage: [
        {
          name: 'zsm-ui',
          value: JSON.stringify({ state: { categoriesFirst: false }, version: 0 }),
        },
      ],
    },
  ],
});

export default defineConfig({
  testDir: 'e2e',
  outputDir: 'test-results/raw',
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  projects: [
    {
      name: 'demo',
      testIgnore: /gmail\.spec\.ts/,
      use: {
        ...devices['Desktop Chrome'],
        baseURL: 'http://localhost:1420',
        storageState: everyone('http://localhost:1420'),
      },
    },
    {
      name: 'gmail',
      testMatch: /gmail\.spec\.ts/,
      use: {
        ...devices['Desktop Chrome'],
        baseURL: 'http://localhost:1430',
        storageState: everyone('http://localhost:1430'),
      },
    },
  ],
  webServer: [
    {
      command:
        'cargo run -q -p zsm-devserver -- --demo --now 2026-11-18T10:00:00+01:00 --tz +01:00',
      url: 'http://127.0.0.1:1421/health',
      reuseExistingServer: true,
      timeout: 240_000,
    },
    {
      command: 'pnpm dev:vite',
      url: 'http://localhost:1420',
      reuseExistingServer: true,
      timeout: 60_000,
    },
    {
      command:
        'cargo run -q -p zsm-devserver -- --fake-google --now 2026-11-18T10:00:00+01:00 --tz +01:00 --port 1422',
      url: 'http://127.0.0.1:1422/health',
      reuseExistingServer: true,
      timeout: 240_000,
    },
    {
      command: 'vite --port 1430 --strictPort',
      env: { ZSM_DEVSERVER_PORT: '1422' },
      url: 'http://localhost:1430',
      reuseExistingServer: true,
      timeout: 60_000,
    },
  ],
});
