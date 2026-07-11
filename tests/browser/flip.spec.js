// FlipItem's reorder glide, on the synchronous DOM-handoff path (`web`
// feature): the wrapper must carry a live transition as soon as the swap is
// observable, and the tiles must land at exchanged positions.
const { test, expect } = require("@playwright/test");

test("FlipItem arms a real transition on swap and glides to the new slot", async ({
  page,
}) => {
  await page.goto("/dioxus-dnd/");
  const a = page.locator("#flip-A");
  const b = page.locator("#flip-B");
  await a.scrollIntoViewIfNeeded();
  const ax = (await a.boundingBox()).x;
  const bx = (await b.boundingBox()).x;
  expect(ax).toBeLessThan(bx);

  await page.click("#flip-swap");

  // The handoff is synchronous, so a CSS transition (a document animation)
  // must be running on the FLIP wrapper well before the 600ms glide ends.
  await expect
    .poll(() => a.evaluate((el) => el.parentElement.getAnimations().length), {
      timeout: 500,
    })
    .toBeGreaterThan(0);

  // And the tiles settle at exchanged positions.
  await expect.poll(async () => (await a.boundingBox()).x).toBe(bx);
  await expect.poll(async () => (await b.boundingBox()).x).toBe(ax);
});
