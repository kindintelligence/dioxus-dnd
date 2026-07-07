const { defineConfig } = require("@playwright/test");

module.exports = defineConfig({
  testDir: "tests/browser",
  timeout: 60_000,
  use: {
    baseURL: "http://127.0.0.1:8080",
    trace: "on-first-retry",
  },
  webServer: [
    {
      command:
        "dx serve --example gallery --platform web --features web --interactive false --open false --hot-patch false --port 8080",
      url: "http://127.0.0.1:8080/dioxus-dnd/",
      timeout: 10 * 60 * 1000,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
    },
    {
      command:
        "dx serve --example canvas --platform web --features web --interactive false --open false --hot-patch false --port 8081",
      url: "http://127.0.0.1:8081/dioxus-dnd/",
      timeout: 10 * 60 * 1000,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
    },
    {
      command:
        "dx serve --example showcase --platform web --features web --interactive false --open false --hot-patch false --port 8082",
      url: "http://127.0.0.1:8082/dioxus-dnd/",
      timeout: 10 * 60 * 1000,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
    },
  ],
});
