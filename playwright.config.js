const { defineConfig } = require("@playwright/test");

module.exports = defineConfig({
  testDir: "tests/browser",
  timeout: 60_000,
  // Browser drag E2E depends on the wasm app hydrating and its CSS settling;
  // under CI load that readiness can lag and time out a test's setup. A retry
  // absorbs those intermittent flakes - the trace is captured on the first
  // retry for diagnosis.
  retries: process.env.CI ? 2 : 1,
  use: {
    baseURL: "http://127.0.0.1:8080",
    trace: "on-first-retry",
    // The fixtures page is tall; a short viewport pushes lower fixtures
    // off-screen where pointer coordinates don't land. Give tests room, and
    // still scroll the target section into view before dragging.
    viewport: { width: 1280, height: 1200 },
  },
  webServer: [
    {
      command:
        "dx serve --example regressions --platform web --features web --interactive false --open false --hot-patch false --port 8080",
      url: "http://127.0.0.1:8080/dioxus-dnd/",
      timeout: 10 * 60 * 1000,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
    },
  ],
});
