// Focus discipline: the library suppresses the browser's incidental focus
// behaviors (a tabindex div is mouse-focusable as a side effect) and adds
// back the one piece of focus that IS wanted - keyboard drops walk focus to
// the moved item's new element.
const { test, expect } = require("@playwright/test");

const activeElement = (page) =>
  page.evaluate(() => {
    const ae = document.activeElement;
    return ae === document.body
      ? "body"
      : (ae.textContent || "").slice(0, 30);
  });

test("a mouse press and drop never focus the draggable", async ({ page }) => {
  await page.goto("/dioxus-dnd/");
  const source = page.locator("#ms-drag");
  await source.scrollIntoViewIfNeeded();
  const src = await source.boundingBox();
  const zone = await page.locator(".ms-zone").boundingBox();

  await page.mouse.move(src.x + 20, src.y + 10);
  await page.mouse.down();
  expect(await activeElement(page)).toBe("body");
  await page.mouse.move(zone.x + 30, zone.y + 20, { steps: 8 });
  await page.mouse.up();
  await page.waitForTimeout(300);
  expect(await activeElement(page)).toBe("body");
});

test("a keyboard drop walks focus to the moved item's new element", async ({
  page,
}) => {
  await page.goto("/dioxus-dnd/");
  const block = page.locator("[aria-roledescription=draggable]", {
    hasText: "Button",
  });
  await block.scrollIntoViewIfNeeded();

  // Keyboard flow: focus, Enter picks up, arrows walk zones, Enter drops.
  await block.evaluate((el) => el.focus());
  await page.keyboard.press("Enter");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");

  // The block re-mounts wherever it landed; focus must follow it there
  // rather than dying on <body>.
  await expect.poll(() => activeElement(page)).toContain("Button");
  expect(
    await block.evaluate((el) => el === document.activeElement)
  ).toBe(true);
});
