// Browser regression suite. Every test drives the headless fixtures in
// examples/regressions.rs (one dev server, stable hooks), so the suite is
// independent of the gallery site's design.
const { test, expect } = require("@playwright/test");

async function section(page, title) {
  return page.locator("section", { has: page.getByRole("heading", { name: title }) });
}

async function sortableRows(page) {
  const sortable = await section(page, "Sortable list");
  return sortable.locator(":scope > div").last().locator(":scope > div");
}

async function sortableRowTexts(page) {
  return (await sortableRows(page)).allInnerTexts();
}

async function gridTileTexts(page) {
  const grid = await section(page, "Grid");
  return grid.evaluate((root) => {
    const container = Array.from(root.querySelectorAll("div")).find(
      (node) => window.getComputedStyle(node).display === "grid",
    );
    if (!container) return [];
    return Array.from(container.children).map((child) => child.textContent.trim());
  });
}

async function elementBox(scope, text, position = null) {
  return scope.evaluate(
    (root, { text, position }) => {
      const element = Array.from(root.querySelectorAll("div")).find((node) => {
        const style = window.getComputedStyle(node);
        return (
          node.textContent.trim() === text &&
          node.childElementCount === 0 &&
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

async function openFixtures(page) {
  await page.goto("/dioxus-dnd/", { waitUntil: "domcontentloaded" });
  await page.addStyleTag({
    content:
      '[id^="__dx-toast"], .dx-toast { display: none !important; pointer-events: none !important; }',
  });
  await expect(page.getByRole("heading", { name: "Regressions" })).toBeVisible({
    timeout: 60_000,
  });
  // Wait for the wasm app to hydrate and layout to settle before measuring.
  await expect
    .poll(
      async () => {
        const sortable = await section(page, "Sortable list");
        const box = await elementBox(sortable, "Research");
        return box ? Math.round(box.width) : 0;
      },
      { timeout: 60_000 },
    )
    .toBeGreaterThan(0);
}

async function dispatchNativeDrop(target, setup) {
  return target.evaluate((node, setup) => {
    const rect = node.getBoundingClientRect();
    const dataTransfer = new DataTransfer();
    if (setup.file) {
      dataTransfer.items.add(
        new File([setup.file.body], setup.file.name, { type: setup.file.type }),
      );
    }
    for (const [type, value] of Object.entries(setup.data || {})) {
      dataTransfer.setData(type, value);
    }
    const init = {
      bubbles: true,
      cancelable: true,
      clientX: rect.left + rect.width / 2,
      clientY: rect.top + rect.height / 2,
      dataTransfer,
    };
    for (const type of ["dragenter", "dragover", "drop"]) {
      node.dispatchEvent(new DragEvent(type, init));
    }
  }, setup);
}

test("sortable overlay matches the source row and cleans up after drop", async ({ page }) => {
  await openFixtures(page);

  const sortable = await section(page, "Sortable list");
  await sortable.scrollIntoViewIfNeeded();
  const sourceBox = await elementBox(sortable, "Research");
  const targetBox = await elementBox(sortable, "Revise");
  expect(sourceBox).not.toBeNull();
  expect(targetBox).not.toBeNull();

  await page.mouse.move(sourceBox.x + sourceBox.width / 2, sourceBox.y + sourceBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(sourceBox.x + sourceBox.width / 2, targetBox.y + targetBox.height * 0.75, {
    steps: 24,
  });
  // Wait for the drag to actually register before inspecting the overlay,
  // rather than a fixed timeout that can lose the race under load.
  await expect(sortable.locator("[data-dragging]").first()).toBeVisible();
  await page.waitForTimeout(100);

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

// A pointer drag released outside the list/grid must commit no reorder.
// (Regression for the pointer path previously snapping to the last-hovered
// target.)
test("sortable release outside the list commits no reorder", async ({ page }) => {
  await openFixtures(page);

  const before = await sortableRowTexts(page);
  expect(before).toEqual(["Research", "Draft", "Review", "Revise", "Publish"]);

  const sortable = await section(page, "Sortable list");
  await sortable.scrollIntoViewIfNeeded();
  const source = await elementBox(sortable, "Research");
  expect(source).not.toBeNull();

  await page.mouse.move(source.x + source.width / 2, source.y + source.height / 2);
  await page.mouse.down();
  // cross the threshold and confirm the drag actually started...
  await page.mouse.move(source.x + source.width / 2, source.y + source.height * 1.6, { steps: 6 });
  await expect(sortable.locator("[data-dragging]").first()).toBeVisible();
  // ...then release at the top-left corner, clearly outside the list bounds.
  await page.mouse.move(5, 5, { steps: 24 });
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.waitForTimeout(300);

  expect(await sortableRowTexts(page)).toEqual(before);
});

test("sortable release inside the list still reorders (control)", async ({ page }) => {
  await openFixtures(page);

  const sortable = await section(page, "Sortable list");
  await sortable.scrollIntoViewIfNeeded();
  const source = await elementBox(sortable, "Research");
  const target = await elementBox(sortable, "Revise");
  expect(source).not.toBeNull();
  expect(target).not.toBeNull();

  await page.mouse.move(source.x + source.width / 2, source.y + source.height / 2);
  await page.mouse.down();
  await page.mouse.move(source.x + source.width / 2, source.y + source.height * 1.6, { steps: 6 });
  await expect(sortable.locator("[data-dragging]").first()).toBeVisible();
  await page.mouse.move(source.x + source.width / 2, target.y + target.height * 0.75, {
    steps: 24,
  });
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.waitForTimeout(300);

  // Research moved down past Revise.
  await expect
    .poll(() => sortableRowTexts(page))
    .toEqual(["Draft", "Review", "Revise", "Research", "Publish"]);
});

test("grid release outside the tiles commits no reorder", async ({ page }) => {
  await openFixtures(page);

  const before = await gridTileTexts(page);
  expect(before.slice(0, 3)).toEqual(["Tile 1", "Tile 2", "Tile 3"]);

  const grid = await section(page, "Grid");
  await grid.scrollIntoViewIfNeeded();
  const source = await elementBox(grid, "Tile 1");
  expect(source).not.toBeNull();

  await page.mouse.move(source.x + source.width / 2, source.y + source.height / 2);
  await page.mouse.down();
  await page.mouse.move(source.x + source.width * 1.6, source.y + source.height / 2, { steps: 6 });
  await expect(grid.locator("[data-dragging]").first()).toBeVisible();
  // release at the top-left corner, clearly outside the grid bounds.
  await page.mouse.move(5, 5, { steps: 24 });
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.waitForTimeout(300);

  expect(await gridTileTexts(page)).toEqual(before);
});

test("autoscroll follows default mouse pointer drags near the edge", async ({ page }) => {
  await openFixtures(page);

  const demo = await section(page, "Autoscroll");
  const scroll = demo.locator(".list-scroll");
  await expect(scroll).toBeVisible();
  await scroll.scrollIntoViewIfNeeded();

  await scroll.evaluate((node) => {
    node.scrollTop = 0;
  });

  const box = await scroll.boundingBox();
  expect(box).not.toBeNull();
  const edgeX = box.x + box.width / 2;
  const edgeY = box.y + box.height - 3;

  // Passive hover near the edge must not scroll.
  await page.mouse.move(edgeX, edgeY);
  await page.waitForTimeout(150);
  expect(await scroll.evaluate((node) => node.scrollTop)).toBe(0);

  const handle = scroll.locator("[data-sort-handle]").first();
  const handleBox = await handle.boundingBox();
  expect(handleBox).not.toBeNull();

  await page.mouse.move(handleBox.x + handleBox.width / 2, handleBox.y + handleBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(
    handleBox.x + handleBox.width / 2 + 24,
    handleBox.y + handleBox.height / 2 + 24,
    { steps: 5 },
  );
  await expect(
    scroll.locator('[data-dragging="true"]').filter({ hasText: "Unload the truck" }).first(),
  ).toBeVisible();

  for (let i = 0; i < 12; i += 1) {
    await page.mouse.move(edgeX, edgeY - (i % 2), { steps: 2 });
    await page.waitForTimeout(25);
  }

  await expect
    .poll(async () => scroll.evaluate((node) => node.scrollTop), { timeout: 5_000 })
    .toBeGreaterThan(0);

  await page.mouse.up();
});

// Autoscroll must not run away: once the pointer leaves the container,
// scrolling stops even though (under pointer capture) move events keep
// bubbling in.
test("autoscroll stops when the pointer leaves the container", async ({ page }) => {
  await openFixtures(page);

  const demo = await section(page, "Autoscroll");
  const scroll = demo.locator(".list-scroll");
  await expect(scroll).toBeVisible();
  await scroll.scrollIntoViewIfNeeded();
  await scroll.evaluate((node) => {
    node.scrollTop = 0;
  });

  const box = await scroll.boundingBox();
  expect(box).not.toBeNull();

  // Start a real pointer drag from the grip.
  const handle = scroll.locator("[data-sort-handle]").first();
  const handleBox = await handle.boundingBox();
  expect(handleBox).not.toBeNull();
  await page.mouse.move(handleBox.x + handleBox.width / 2, handleBox.y + handleBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(
    handleBox.x + handleBox.width / 2 + 24,
    handleBox.y + handleBox.height / 2 + 24,
    { steps: 5 },
  );
  await expect(
    scroll.locator('[data-dragging="true"]').filter({ hasText: "Unload the truck" }).first(),
  ).toBeVisible();

  // Move the pointer far ABOVE the container (outside it) and keep nudging it
  // there. Pre-fix this pinned the delta to full speed and scrolled anyway.
  const outsideX = box.x + box.width / 2;
  const outsideY = box.y - 200;
  for (let i = 0; i < 12; i += 1) {
    await page.mouse.move(outsideX, outsideY - (i % 2), { steps: 2 });
    await page.waitForTimeout(25);
  }
  expect(await scroll.evaluate((node) => node.scrollTop)).toBe(0);

  await page.mouse.up();
});

test("canvas pointer drop uses the recorded grab offset", async ({ page }) => {
  await openFixtures(page);

  const canvas = await section(page, "Canvas");
  const node = canvas.getByText("Input", { exact: true });
  await node.scrollIntoViewIfNeeded();
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
  await page.mouse.move(startX + 20, startY + 20, { steps: 5 });
  await expect(
    canvas.locator('[data-dragging="true"]').filter({ hasText: "Input" }).first(),
  ).toBeVisible();
  await page.mouse.move(endX, endY, { steps: 30 });
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.waitForTimeout(300);

  const after = await node.boundingBox();
  expect(after).not.toBeNull();
  expect(Math.abs(after.x - before.x)).toBeGreaterThan(40);
  expect(Math.abs(after.y - before.y)).toBeGreaterThan(30);
});

// A pointer drop over a zone that rejects the payload must fall through to an
// accepting zone stacked underneath, not cancel. (Regression for finish_drop
// cancelling when the geometric-topmost zone rejected.)
test("pointer drop falls through a rejecting zone to the accepting one under it", async ({
  page,
}) => {
  await openFixtures(page);

  const status = page.locator("#overlap-status");
  await expect(status).toHaveAttribute("data-landed", "none");

  const source = page.getByText("drag me", { exact: true });
  const stack = page.locator("#overlap-stack");
  const sb = await source.boundingBox();
  const tb = await stack.boundingBox();
  expect(sb).not.toBeNull();
  expect(tb).not.toBeNull();

  await page.mouse.move(sb.x + sb.width / 2, sb.y + sb.height / 2);
  await page.mouse.down();
  await page.mouse.move(sb.x + sb.width / 2, sb.y + sb.height / 2 + 12, { steps: 5 });
  await page.mouse.move(tb.x + tb.width / 2, tb.y + tb.height / 2, { steps: 20 });
  await page.waitForTimeout(100);
  await page.mouse.up();

  // The drop reached the accepting zone under the rejecting one.
  await expect(status).toHaveAttribute("data-landed", "accept", { timeout: 5_000 });
});

// The pointer path must honor the modifier-key convention (Ctrl/Cmd = copy),
// so a Ctrl-drag leaves the source in place.
test("ctrl-drag on the pointer path copies instead of moving", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Copy vs move");
  await sec.scrollIntoViewIfNeeded();
  const palette = sec.getByText("Palette", { exact: true }).locator("xpath=ancestor::div[1]");
  const stage = sec.getByText("Stage", { exact: true }).locator("xpath=ancestor::div[1]");
  const paletteCount = () => palette.getByText(/^(Button|Input|Chart)$/).count();

  expect(await paletteCount()).toBe(3);

  const src = await sec.getByText("Button", { exact: true }).boundingBox();
  const dst = await stage.boundingBox();
  expect(src).not.toBeNull();
  expect(dst).not.toBeNull();

  await page.keyboard.down("Control");
  await page.mouse.move(src.x + src.width / 2, src.y + src.height / 2);
  await page.mouse.down();
  await page.mouse.move(src.x + src.width / 2 + 10, src.y + src.height / 2 + 10, { steps: 5 });
  await page.mouse.move(dst.x + dst.width / 2, dst.y + dst.height / 2, { steps: 20 });
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.keyboard.up("Control");
  await page.waitForTimeout(300);

  // Copy: the source stays in the palette AND a copy lands on the stage.
  expect(await paletteCount()).toBe(3);
  await expect(stage.getByText("Button", { exact: true })).toBeVisible();
});

// ReorderButtons rendered inside a SortableList row must still receive
// clicks: pressing one must not let the row grab pointer capture and swallow
// the click.
test("reorder buttons reorder from inside a sortable row", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Accessible reorder");
  const order = () =>
    sec
      .locator("span")
      .filter({ hasText: /^(Wake up|Ship code|Touch grass|Sleep)$/ })
      .allInnerTexts();

  expect(await order()).toEqual(["Wake up", "Ship code", "Touch grass", "Sleep"]);
  await sec.getByRole("button", { name: "Move Wake up down" }).click();
  await expect.poll(order).toEqual(["Ship code", "Wake up", "Touch grass", "Sleep"]);
});

test("native DataTransfer paths handle files, external drops, and drag-out", async ({ page }) => {
  await openFixtures(page);

  const files = await section(page, "File drop");
  const fileZone = files.getByText("Drop files from your desktop here", { exact: true });
  await dispatchNativeDrop(fileZone, {
    file: { name: "agent-notes.txt", type: "text/plain", body: "native file payload" },
  });
  await expect(files.getByText("agent-notes.txt", { exact: true })).toBeVisible();

  const inOut = await section(page, "In & out");
  const externalDrop = inOut.getByText("Drop text or a link here", { exact: true });
  await dispatchNativeDrop(externalDrop, {
    data: {
      "text/uri-list": "https://dioxuslabs.com\n",
      "text/html": '<a href="https://dioxuslabs.com">Dioxus</a>',
      "text/plain": "https://dioxuslabs.com",
    },
  });
  await expect(inOut.getByText("3 payload(s), 0 file(s)", { exact: true })).toBeVisible();

  const outbound = await inOut
    .getByText(/^Drag this link out/)
    .evaluate((node) => {
      const dataTransfer = new DataTransfer();
      node.dispatchEvent(
        new DragEvent("dragstart", {
          bubbles: true,
          cancelable: true,
          dataTransfer,
        }),
      );
      return {
        uri: dataTransfer.getData("text/uri-list"),
        text: dataTransfer.getData("text/plain"),
        html: dataTransfer.getData("text/html"),
      };
    });
  expect(outbound).toEqual({
    uri: "https://dioxuslabs.com",
    text: "https://dioxuslabs.com",
    html: '<a href="https://dioxuslabs.com">Dioxus</a>',
  });
});

// The bridge pattern (README "Mixing payload types"): one box registered in
// two payload worlds. Each world's drag lights and lands on the shared zone
// through its own typed callback, while the other world's zones stay dark
// and unreachable for the foreign drag.
test("bridge zone receives typed drops from both payload worlds", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Bridge zone");
  await sec.scrollIntoViewIfNeeded();
  const zone = sec.locator("#bridge-zone");
  const ticketOnly = sec.locator(".ticket-only");
  const status = sec.locator("#bridge-status");

  const dragTo = async (sourceId, target) => {
    const from = await sec.locator(sourceId).boundingBox();
    const to = await target.boundingBox();
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 20 });
  };

  // A ticket drag activates its own world: the ticket zone AND the bridge.
  await dragTo("#bridge-ticket", zone);
  await expect(zone).toHaveAttribute("data-active", "true");
  await expect(zone).toHaveAttribute("data-over", "true");
  await expect(ticketOnly).toHaveAttribute("data-active", "true");
  await page.mouse.up();
  await expect(status).toHaveAttribute("data-log", "ticket:DND-41");

  // A person drag activates the bridge but NOT the ticket-only zone.
  await dragTo("#bridge-person", zone);
  await expect(zone).toHaveAttribute("data-active", "true");
  await expect(ticketOnly).not.toHaveAttribute("data-active", "true");
  await page.mouse.up();
  await expect(status).toHaveAttribute("data-log", "ticket:DND-41,person:7");

  // A person dropped on the ticket-only zone cancels: wrong world entirely.
  await dragTo("#bridge-person", ticketOnly);
  await page.mouse.up();
  await expect(status).toHaveAttribute("data-log", "ticket:DND-41,person:7");
});

// Zone rects are cached at drag start; autoscroll moves the zones mid-drag.
// The rect-refresh channel re-measures after every scroll, so both the hover
// highlight and the drop must target the zone the user actually sees at the
// pointer - not the one whose stale rect still covers that point.
test("drops land on the zone that auto-scrolled into place", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Stale rects");
  await sec.scrollIntoViewIfNeeded();
  const scroll = sec.locator(".stale-scroll");
  await scroll.evaluate((node) => {
    node.scrollTop = 0;
  });

  const drag = sec.locator("#stale-drag");
  const d = await drag.boundingBox();
  const box = await scroll.boundingBox();

  // Pick up the item and park near the container's bottom edge so
  // autoscroll kicks in; keep feeding pointermove so it keeps ticking.
  await page.mouse.move(d.x + d.width / 2, d.y + d.height / 2);
  await page.mouse.down();
  const edgeX = box.x + box.width / 2;
  const edgeY = box.y + box.height - 4;
  await page.mouse.move(edgeX, edgeY, { steps: 15 });
  let jiggle = 0;
  await expect
    .poll(async () => {
      jiggle = 1 - jiggle;
      await page.mouse.move(edgeX + jiggle, edgeY);
      return scroll.evaluate((node) => node.scrollTop);
    })
    .toBeGreaterThan(120);

  // The list has scrolled well past a full zone height. Ask the DOM which
  // zone is REALLY under the container's midpoint now...
  const midX = box.x + box.width / 2;
  const midY = box.y + box.height / 2;
  const expected = await page.evaluate(([x, y]) => {
    const el = document.elementFromPoint(x, y);
    return el?.closest(".stale-zone")?.textContent.trim() ?? null;
  }, [midX, midY]);
  expect(expected).not.toBeNull();

  // ...hover it: the freshly-measured zone must light up, not its stale
  // predecessor...
  await page.mouse.move(midX, midY, { steps: 8 });
  const target = sec.locator(".stale-zone").filter({ hasText: new RegExp(`^${expected}$`) });
  await expect(target).toHaveAttribute("data-over", "true");

  // ...and the drop must land there.
  await page.mouse.up();
  await expect(sec.locator("#stale-status")).toHaveAttribute("data-landed", expected);
});
