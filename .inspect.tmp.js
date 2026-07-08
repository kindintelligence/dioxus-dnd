const { chromium } = require("@playwright/test");
(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await page.goto("http://127.0.0.1:8081/dioxus-dnd/playlist", { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(4000);
  const info = await page.evaluate(() => {
    const els = Array.from(document.querySelectorAll("*")).filter(
      (n) => n.textContent?.includes("prefers-reduced-motion") && n.children.length === 0,
    );
    return els.map((n) => ({
      tag: n.tagName,
      ns: n.namespaceURI,
      display: getComputedStyle(n).display,
      parentTag: n.parentElement?.tagName,
      outer: n.outerHTML.slice(0, 160),
    }));
  });
  console.log(JSON.stringify(info, null, 2));
  await page.screenshot({ path: "/tmp/playlist-bug.png" });
  await browser.close();
})();
