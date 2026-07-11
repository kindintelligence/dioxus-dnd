// Touch-path specs, driven through CDP's Input.dispatchTouchEvent so the
// gestures run Chromium's real input pipeline (scroll gesture recognition,
// pointercancel, passive-listener semantics) - page.touchscreen only taps.
const { test, expect } = require("@playwright/test");

test.use({ hasTouch: true });

// One finger down at (x, y), dragged by (dx, dy) over `steps` moves, then
// lifted. Small per-step waits let the compositor run its gesture recognizer
// between moves, as a real finger would.
async function swipe(page, x, y, dx, dy, { steps = 10, holdMs = 0 } = {}) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [{ x, y }],
  });
  if (holdMs) {
    await page.waitForTimeout(holdMs);
  }
  for (let i = 1; i <= steps; i++) {
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchMove",
      touchPoints: [{ x: x + (dx * i) / steps, y: y + (dy * i) / steps }],
    });
    await page.waitForTimeout(16);
  }
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await cdp.detach();
}

async function centerOf(page, selector) {
  const box = await page.locator(selector).boundingBox();
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

// Row N of the touch-sense list (sibling styles and the transient hold-timer
// element make :nth-child unreliable).
async function tsRowCenter(page, n) {
  const box = await page
    .locator("#ts-scroll [data-dnd-motion]")
    .nth(n)
    .boundingBox();
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

// Spike pin: a synchronous ontouchmove handler calling prevent_default()
// must cancel the native pan (dioxus-web's delegated listener on #main is
// non-passive), while an identical lane without the handler scrolls freely.
test("ontouchmove prevent_default blocks native scroll; without it, scroll flows", async ({
  page,
}) => {
  await page.goto("/dioxus-dnd/");
  const scroller = page.locator("#tp-scroll");
  await scroller.scrollIntoViewIfNeeded();

  const scrollTop = () => scroller.evaluate((el) => el.scrollTop);

  // Blocking lane first - the control fling below leaves momentum that
  // would bleed into this lane's stillness assertion.
  const blocker = await centerOf(page, "#tp-blocker");
  await swipe(page, blocker.x, blocker.y, 0, -80);
  // Give any wrongly-started pan time to land before asserting stillness.
  await page.waitForTimeout(200);
  expect(await scrollTop()).toBe(0);

  // Control lane: prove the identical gesture scrolls without the handler.
  const free = await centerOf(page, "#tp-free");
  await swipe(page, free.x, free.y, 0, -80);
  await expect.poll(scrollTop).toBeGreaterThan(0);
});

// --- TouchSense::Auto on a whole-row SortableList in a scroller --------------

const INITIAL = "Item 1,Item 2,Item 3,Item 4,Item 5,Item 6,Item 7,Item 8,Item 9,Item 10";

async function tsSetup(page) {
  await page.goto("/dioxus-dnd/");
  const scroller = page.locator("#ts-scroll");
  await scroller.scrollIntoViewIfNeeded();
  const status = page.locator("#ts-status");
  await expect(status).toHaveAttribute("data-order", INITIAL);
  return {
    scroller,
    order: () => status.getAttribute("data-order"),
    scrollTop: () => scroller.evaluate((el) => el.scrollTop),
  };
}

test("a vertical swipe scrolls the list and reorders nothing", async ({ page }) => {
  const { order, scrollTop } = await tsSetup(page);
  const row = await tsRowCenter(page, 2);
  await swipe(page, row.x, row.y, 0, -80);
  await expect.poll(scrollTop).toBeGreaterThan(0);
  expect(await order()).toBe(INITIAL);
});

test("a 250ms hold picks the row up; the list stays put and reorders", async ({
  page,
}) => {
  const { order, scrollTop } = await tsSetup(page);
  const row = await tsRowCenter(page, 1);
  // Hold well past the 250ms promotion beat, then pull two 40px rows down.
  await swipe(page, row.x, row.y, 0, 85, { holdMs: 450 });
  await expect
    .poll(order)
    .not.toBe(INITIAL);
  expect(await order()).not.toContain("Item 1,Item 2");
  expect(await scrollTop()).toBe(0);
});

test("the arming hold timer injects no visible text", async ({ page }) => {
  await tsSetup(page);
  const row = await tsRowCenter(page, 1);
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [{ x: row.x, y: row.y }],
  });
  // Mid-hold the timer (style + zero-size div) is mounted; its keyframes
  // sheet must be display:none - dioxus-web renders bare <style> elements
  // visibly, so this pins the guard attribute.
  await page.waitForTimeout(120);
  const display = await page.evaluate(() => {
    const sheet = [...document.querySelectorAll("style")].find((s) =>
      s.textContent.includes("dnd-hold-timer")
    );
    return sheet ? getComputedStyle(sheet).display : "absent";
  });
  expect(display).toBe("none");
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await cdp.detach();
});

test("a sideways pull picks the row up with no hold", async ({ page }) => {
  const { order, scrollTop } = await tsSetup(page);
  const row = await tsRowCenter(page, 1);
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [{ x: row.x, y: row.y }],
  });
  // Sideways first (promotes at |dx| > |dy| past the threshold)...
  for (const dx of [8, 16, 24]) {
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchMove",
      touchPoints: [{ x: row.x + dx, y: row.y }],
    });
    await page.waitForTimeout(16);
  }
  // ...then straight down: the promoted drag owns the touch now.
  for (let i = 1; i <= 8; i++) {
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchMove",
      touchPoints: [{ x: row.x + 24, y: row.y + (85 * i) / 8 }],
    });
    await page.waitForTimeout(16);
  }
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await cdp.detach();
  await expect.poll(order).not.toBe(INITIAL);
  expect(await scrollTop()).toBe(0);
});
