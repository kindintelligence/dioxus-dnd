// Canvas gestures on one node (examples/regressions.rs, CanvasGesturesFixture):
// a click or a double click on a child reaches it, a finished drag fires no
// click, the grab is measured from the node, Escape and the window losing
// focus cancel a pointer drag, only the drag's own pointer cancels it, a second
// finger is a pinch, and a keyboard drag ignores keys from a field and held
// keys, and ends when focus leaves the node.
const { test, expect } = require("@playwright/test");

test.use({ hasTouch: true });

async function open(page) {
  await page.goto("/dioxus-dnd/", { waitUntil: "domcontentloaded" });
  const node = page.locator("#cg-node");
  await expect(node).toBeVisible({ timeout: 60_000 });
  await node.scrollIntoViewIfNeeded();
  return { node, status: page.locator("#cg-status") };
}

async function centre(locator) {
  const box = await locator.boundingBox();
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

// Presses at (x, y) and moves 40px right and down, past the 8px threshold, so
// the press is a drag.
async function startDrag(page, node, x, y) {
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 40, y + 40, { steps: 8 });
  await expect(node).toHaveAttribute("data-dragging", "true");
}

// The pointer was captured at the press, so the browser sent the click to the
// node instead of the button pressed.
test("a click and a double click on a child inside a node reach the child", async ({ page }) => {
  const { status } = await open(page);
  await page.locator("#cg-child").click();
  await expect(status).toHaveAttribute("data-child-clicks", "1");
  await page.locator("#cg-child").dblclick();
  await expect(status).toHaveAttribute("data-child-doubles", "1");
});

test("a finished drag fires no click on the node; a plain press does", async ({ page }) => {
  const { node, status } = await open(page);
  const header = await centre(page.locator("#cg-header"));
  await startDrag(page, node, header.x, header.y);
  await page.mouse.move(header.x + 120, header.y + 60, { steps: 8 });
  await page.mouse.up();
  await expect(status).toHaveAttribute("data-end", "dropped");
  await expect(status).toHaveAttribute("data-clicks", "0");

  await page.locator("#cg-header").click();
  await expect(status).toHaveAttribute("data-clicks", "1");
});

// The grab came from the event's offsetX/offsetY, measured from the child
// pressed: a node grabbed below its 32px header landed 32px too low.
test("a node grabbed below its header lands where its ghost was", async ({ page }) => {
  const { node, status } = await open(page);
  const before = await node.boundingBox();
  const body = await page.locator("#cg-body").boundingBox();
  const press = { x: body.x + 20, y: body.y + 10 };
  const grab = { x: press.x - before.x, y: press.y - before.y };

  await startDrag(page, node, press.x, press.y);
  const release = { x: press.x + 150, y: press.y + 70 };
  await page.mouse.move(release.x, release.y, { steps: 8 });
  await page.mouse.up();
  await expect(status).toHaveAttribute("data-end", "dropped");

  // Within the canvas's 1px border: drop points are measured from its outer
  // edge, places from inside it.
  await expect
    .poll(async () => {
      const after = await node.boundingBox();
      return Math.max(
        Math.abs(after.x - (release.x - grab.x)),
        Math.abs(after.y - (release.y - grab.y)),
      );
    })
    .toBeLessThanOrEqual(2);
});

test("Escape cancels a pointer drag and the node stays", async ({ page }) => {
  const { node, status } = await open(page);
  const before = await node.boundingBox();
  const header = await centre(page.locator("#cg-header"));
  await startDrag(page, node, header.x, header.y);

  await page.keyboard.press("Escape");
  await expect(status).toHaveAttribute("data-end", "cancelled");
  await page.mouse.move(header.x + 150, header.y + 80, { steps: 4 });
  await page.mouse.up();
  expect(await node.boundingBox()).toEqual(before);
});

// It dropped wherever the cursor came back into the window.
test("the window losing focus cancels a pointer drag", async ({ page }) => {
  const { node, status } = await open(page);
  const before = await node.boundingBox();
  const header = await centre(page.locator("#cg-header"));
  await startDrag(page, node, header.x, header.y);

  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await expect(status).toHaveAttribute("data-end", "cancelled");
  await page.mouse.up();
  expect(await node.boundingBox()).toEqual(before);
});

test("a cancel from another pointer leaves the drag", async ({ page }) => {
  const { node, status } = await open(page);
  const header = await centre(page.locator("#cg-header"));
  await startDrag(page, node, header.x, header.y);

  await node.evaluate((element) =>
    element.dispatchEvent(new PointerEvent("pointercancel", { pointerId: 999, bubbles: true })),
  );
  await expect(node).toHaveAttribute("data-dragging", "true");
  await page.mouse.move(header.x + 120, header.y + 60, { steps: 4 });
  await page.mouse.up();
  await expect(status).toHaveAttribute("data-end", "dropped");
});

// It went on as a drag of the node, which moved under the pinch.
test("a second finger makes it a pinch: the drag ends and the node stays", async ({ page }) => {
  const { node, status } = await open(page);
  const before = await node.boundingBox();
  const header = await centre(page.locator("#cg-header"));
  const cdp = await page.context().newCDPSession(page);

  const one = { x: header.x, y: header.y, id: 0 };
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [one] });
  for (let i = 1; i <= 6; i++) {
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchMove",
      touchPoints: [{ ...one, x: one.x + i * 5 }],
    });
    await page.waitForTimeout(16);
  }
  await expect(node).toHaveAttribute("data-dragging", "true");

  const held = { ...one, x: one.x + 30 };
  const two = { x: held.x + 200, y: held.y + 100, id: 1 };
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [held, two] });
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [
      { ...held, x: held.x - 10 },
      { ...two, x: two.x + 10 },
    ],
  });
  await expect(status).toHaveAttribute("data-end", "cancelled");
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await cdp.detach();
  expect(await node.boundingBox()).toEqual(before);
});

// Space typed in the field lifted the node and was swallowed.
test("keys typed in a field inside a node are the field's", async ({ page }) => {
  const { node } = await open(page);
  const field = page.locator("#cg-field");
  await field.focus();
  await page.keyboard.type("a b");
  await page.keyboard.press("Enter");
  await expect(field).toHaveValue("a b");
  await expect(node).not.toHaveAttribute("data-dragging", "true");
});

// The repeat dropped the node into the canvas, its corner at the centre.
test("held, Enter doesn't drop what it lifted", async ({ page }) => {
  const { node, status } = await open(page);
  await node.focus();
  await page.keyboard.down("Enter");
  await expect(node).toHaveAttribute("data-dragging", "true");
  await page.keyboard.down("Enter");
  await page.keyboard.down("Enter");
  await page.keyboard.up("Enter");
  await expect(node).toHaveAttribute("data-dragging", "true");
  await expect(status).toHaveAttribute("data-end", "none");

  await page.keyboard.press("Escape");
  await expect(status).toHaveAttribute("data-end", "cancelled");
});

// The drag stayed live after focus left, and the next Enter elsewhere dropped
// it.
test("Tab during a keyboard drag puts the node back", async ({ page }) => {
  const { node, status } = await open(page);
  const before = await node.boundingBox();
  await node.focus();
  await page.keyboard.press("Space");
  await expect(node).toHaveAttribute("data-dragging", "true");

  await page.keyboard.press("Tab");
  await expect(status).toHaveAttribute("data-end", "cancelled");
  await expect(node).not.toHaveAttribute("data-dragging", "true");
  expect(await node.boundingBox()).toEqual(before);
});
