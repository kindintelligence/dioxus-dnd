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
      // Use the explicit optional-value form and disable Rust hot-patching:
      // the alpha CLI's global settings must not change regression behavior.
      command:
        "dx serve --example regressions --platform web --features web,serde --interactive=false --open=false --hot-patch=false --port 8080",
      // The alpha server returns HTTP 200 for its build-in-progress page.
      // Start tests only after the fixture build is ready to launch.
      wait: {
        stdout: /Build completed successfully.*launching app/,
        stderr: /Build completed successfully.*launching app/,
      },
      timeout: 10 * 60 * 1000,
      stdout: "pipe",
    },
  ],
});
