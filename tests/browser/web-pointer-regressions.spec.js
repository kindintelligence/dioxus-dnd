const { test, expect } = require("@playwright/test");

async function section(page, title) {
  return page.locator("section", { has: page.getByRole("heading", { name: title }) });
}

async function sortableRows(page) {
  const sortable = await section(page, "Sortable list");
  return sortable.locator(":scope > div").last().locator(":scope > div");
}

async function elementBox(scope, text, position = null) {
  return scope.evaluate(
    (root, { text, position }) => {
      const element = Array.from(root.querySelectorAll("div")).find((node) => {
        const style = window.getComputedStyle(node);
        return (
          node.textContent.trim() === text &&
          (!position || style.position === position)
        );
      });
      if (!element) {
        return null;
      }
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        opacity: style.opacity,
        position: style.position,
        childElementCount: element.childElementCount,
      };
    },
    { text, position },
  );
}

async function openGallery(page) {
  await page.goto("/dioxus-dnd/", { waitUntil: "domcontentloaded" });
  await expect(page.getByRole("heading", { name: "Drag & drop gallery" })).toBeVisible({
    timeout: 60_000,
  });
}

test("sortable overlay matches the source row and cleans up after drop", async ({ page }) => {
  await openGallery(page);

  const sortable = await section(page, "Sortable list");
  const sourceBox = await elementBox(sortable, "Research");
  const targetBox = await elementBox(sortable, "Revise");
  expect(sourceBox).not.toBeNull();
  expect(targetBox).not.toBeNull();

  await page.mouse.move(sourceBox.x + sourceBox.width / 2, sourceBox.y + sourceBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(sourceBox.x + sourceBox.width / 2, targetBox.y + targetBox.height * 0.75, {
    steps: 24,
  });
  await page.waitForTimeout(250);

  const overlayBox = await elementBox(sortable, "Research", "fixed");
  expect(overlayBox).not.toBeNull();
  expect(overlayBox.position).toBe("fixed");
  expect(overlayBox.childElementCount).toBe(0);
  expect(Math.round(overlayBox.width)).toBe(Math.round(sourceBox.width));
  expect(Math.round(overlayBox.height)).toBe(Math.round(sourceBox.height));

  await page.mouse.up();
  await page.waitForTimeout(300);

  const rows = await sortableRows(page);
  await expect(rows).toHaveCount(5);
  for (let index = 0; index < 5; index += 1) {
    await expect(rows.nth(index)).not.toHaveCSS("position", "fixed");
    await expect(rows.nth(index)).toHaveCSS("opacity", "1");
  }
});

test("canvas pointer drop uses the recorded grab offset", async ({ page }) => {
  await openGallery(page);

  const canvas = await section(page, "Canvas");
  const node = canvas.getByText("Input", { exact: true });
  const canvasBox = await canvas.locator(".relative").boundingBox();
  const before = await node.boundingBox();
  expect(canvasBox).not.toBeNull();
  expect(before).not.toBeNull();

  const startX = before.x + before.width / 2;
  const startY = before.y + before.height / 2;
  const endX = canvasBox.x + canvasBox.width * 0.65;
  const endY = canvasBox.y + canvasBox.height * 0.65;

  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(endX, endY, { steps: 30 });
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.waitForTimeout(300);

  const after = await node.boundingBox();
  expect(after).not.toBeNull();
  expect(Math.abs(after.x - before.x)).toBeGreaterThan(40);
  expect(Math.abs(after.y - before.y)).toBeGreaterThan(30);
});
