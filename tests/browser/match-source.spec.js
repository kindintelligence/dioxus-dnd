// DragOverlay { match_source } + on_settled: the ghost wears the grabbed
// element's rect so the cursor stays inside it wherever you grab, and the
// settle-completion callback fires exactly once, after the glide.
const { test, expect } = require("@playwright/test");

test("match_source ghost adopts the source size and stays under the cursor", async ({
  page,
}) => {
  await page.goto("/dioxus-dnd/");
  const source = page.locator("#ms-drag");
  await source.scrollIntoViewIfNeeded();
  const src = await source.boundingBox();

  // Grab near the RIGHT edge - the case that used to fling a
  // content-sized ghost far off to the left of the cursor.
  const grabX = src.x + src.width - 15;
  const grabY = src.y + src.height / 2;
  await page.mouse.move(grabX, grabY);
  await page.mouse.down();
  await page.mouse.move(grabX + 30, grabY + 30, { steps: 4 });

  const ghost = page.locator(".ms-ghost");
  await expect(ghost).toBeVisible();
  const g = await ghost.boundingBox();
  // Same size as the source (within a pixel of rounding)...
  expect(Math.abs(g.width - src.width)).toBeLessThanOrEqual(1);
  expect(Math.abs(g.height - src.height)).toBeLessThanOrEqual(1);
  // ...and the cursor sits inside the ghost, exactly where the grab was.
  const cursor = { x: grabX + 30, y: grabY + 30 };
  expect(cursor.x).toBeGreaterThanOrEqual(g.x);
  expect(cursor.x).toBeLessThanOrEqual(g.x + g.width);
  expect(cursor.y).toBeGreaterThanOrEqual(g.y);
  expect(cursor.y).toBeLessThanOrEqual(g.y + g.height);

  await page.mouse.up();
});

test("on_settled fires once, after the glide completes", async ({ page }) => {
  await page.goto("/dioxus-dnd/");
  const source = page.locator("#ms-drag");
  await source.scrollIntoViewIfNeeded();
  const src = await source.boundingBox();
  const zone = await page.locator(".ms-zone").boundingBox();
  const status = page.locator("#ms-status");
  await expect(status).toHaveAttribute("data-settled", "0");

  await page.mouse.move(src.x + 20, src.y + 10);
  await page.mouse.down();
  // Release near the zone's top edge: far enough from its center that the
  // settle glide is a real transition, not the sub-pixel fast path.
  await page.mouse.move(zone.x + 30, zone.y + 8, { steps: 8 });
  await page.mouse.up();

  // Not yet - the ghost is still gliding (default 200ms). Meanwhile the
  // landed element already holds its space, but invisibly: one object on
  // screen, not a copy beside the ghost.
  expect(await status.getAttribute("data-settled")).toBe("0");
  const landed = page.locator("#ms-landed");
  await expect(landed).toHaveAttribute("data-settling", "true");
  expect(await landed.evaluate((el) => getComputedStyle(el).visibility)).toBe(
    "hidden"
  );

  await expect(status).toHaveAttribute("data-settled", "1", { timeout: 2000 });
  // And exactly once: no double-fire from stray transitionends.
  await page.waitForTimeout(300);
  expect(await status.getAttribute("data-settled")).toBe("1");

  // The swap: ghost gone, landed element revealed, and the glide ended on
  // the landed element's own rect (retargeted), not the zone's center.
  await expect(landed).not.toHaveAttribute("data-settling", "true");
  expect(await landed.evaluate((el) => getComputedStyle(el).visibility)).toBe(
    "visible"
  );
  await expect(page.locator(".ms-ghost")).toHaveCount(0);
});
