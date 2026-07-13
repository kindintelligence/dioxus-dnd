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
  const fileZone = files.getByText("Click to choose files or drop them here", { exact: true });

  const chooserPromise = page.waitForEvent("filechooser");
  await fileZone.click();
  const chooser = await chooserPromise;
  expect(chooser.isMultiple()).toBe(true);
  await chooser.setFiles({
    name: "picker-notes.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("native picker payload"),
  });
  const pickerNotes = files.getByText("picker-notes.txt", { exact: true });
  await expect(pickerNotes).toHaveCount(1);

  const repeatedChooserPromise = page.waitForEvent("filechooser");
  await fileZone.click();
  const repeatedChooser = await repeatedChooserPromise;
  await repeatedChooser.setFiles({
    name: "picker-notes.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("native picker payload"),
  });
  await expect(pickerNotes).toHaveCount(2);

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

// The bridge pattern (README "Mixing payload types"), shipped as
// `BridgeDropZone<A, B>`: one box registered in two payload worlds. Each
// world's drag lights and lands on the shared zone through its own typed
// callback, while the other world's zones stay dark and unreachable for the
// foreign drag.
test("bridge zone receives typed drops from both payload worlds", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Bridge zone");
  await sec.scrollIntoViewIfNeeded();
  const zone = sec.locator(".bridge-zone");
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

// SortableList's row rects are measured at drag start; autoscroll shifts
// the rows under the pointer mid-drag. The compensated re-measure (base
// slot = transformed measurement minus the preview displacement we applied)
// must make the drop land where the user sees it: dropping at 70% depth of
// slot t - computed from the live scroll offset - puts the dragged row at
// index t, not at the slot that USED to be there before scrolling.
test("sortable reorders against rows that auto-scrolled into place", async ({ page }) => {
  await openFixtures(page);

  const demo = await section(page, "Autoscroll");
  const scroll = demo.locator(".list-scroll");
  await scroll.scrollIntoViewIfNeeded();
  await scroll.evaluate((node) => {
    node.scrollTop = 0;
  });

  const rowTexts = () => demo.locator("[data-sort-content]").allInnerTexts();
  expect((await rowTexts())[0]).toBe("Unload the truck");

  // Slot geometry before any drag: pitch from the first two rows, and the
  // first slot's client top at scrollTop = 0.
  const contents = demo.locator("[data-sort-content]");
  const r0 = await contents.nth(0).boundingBox();
  const r1 = await contents.nth(1).boundingBox();
  const pitch = r1.y - r0.y;
  const box = await scroll.boundingBox();

  // Pick up row 0 by its handle, park at the bottom edge until the list
  // has scrolled well past a couple of slots.
  const handle = scroll.locator("[data-sort-handle]").first();
  const hb = await handle.boundingBox();
  await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2);
  await page.mouse.down();
  const edgeX = box.x + box.width / 2;
  const edgeY = box.y + box.height - 4;
  await page.mouse.move(edgeX, edgeY, { steps: 12 });
  let jiggle = 0;
  await expect
    .poll(async () => {
      jiggle = 1 - jiggle;
      await page.mouse.move(edgeX + jiggle, edgeY);
      return scroll.evaluate((node) => node.scrollTop);
    })
    .toBeGreaterThan(120);

  // Leave the edge band so scrolling stops, let in-flight scroll tasks
  // settle, then read the final offset the drop must be judged against.
  await page.mouse.move(edgeX, box.y + box.height / 2, { steps: 4 });
  await page.waitForTimeout(250);
  const scrolled = await scroll.evaluate((node) => node.scrollTop);

  // Choose the slot whose 70% depth sits nearest the container middle and
  // drop exactly there: past the midpoint, so the crossing rule adopts it.
  const midY = box.y + box.height / 2;
  const t = Math.max(1, Math.round((midY - (r0.y - scrolled)) / pitch - 0.7));
  const dropY = r0.y - scrolled + (t + 0.7) * pitch;
  await page.mouse.move(edgeX, dropY, { steps: 6 });
  await page.mouse.up();

  const after = await rowTexts();
  expect(after[t]).toBe("Unload the truck");
});

// Drop-settle (DragOverlay { settle: true }): a successful pointer drop keeps
// the ghost alive and glides it into the receiving zone, then unmounts it on
// transitionend. The drag itself is already over - the zone unlights and the
// drop handler ran at release. A cancelled drag never settles.
test("drop-settle glides the ghost into the zone then unmounts it", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Drop settle");
  await sec.scrollIntoViewIfNeeded();
  const zone = sec.locator(".settle-zone");
  const ghost = page.locator(".settle-ghost");
  const status = sec.locator("#settle-status");

  const from = await sec.locator("#settle-drag").boundingBox();
  const to = await zone.boundingBox();
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  // Release near the zone's top-left corner so the glide toward its center
  // has real distance to cover.
  await page.mouse.move(to.x + 15, to.y + 10, { steps: 20 });
  await page.mouse.up();

  // The ghost survives the drop to run the glide, marked for the
  // reduced-motion override...
  await expect(ghost).toBeVisible();
  await expect(ghost).toHaveAttribute("data-dnd-motion", "true");
  // ...while the drag itself ended at release: handler ran, zone unlit.
  await expect(status).toHaveAttribute("data-landed", "landed:5");
  await expect(zone).not.toHaveAttribute("data-active", "true");
  // The transition ends and the ghost unmounts.
  await expect(ghost).toHaveCount(0, { timeout: 3000 });

  // A cancelled drag (released over nothing) vanishes without settling.
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 60, from.y - 80, { steps: 10 });
  await expect(ghost).toBeVisible();
  await page.mouse.up();
  await expect(ghost).toHaveCount(0, { timeout: 1000 });
  await expect(status).toHaveAttribute("data-landed", "landed:5");
});

// Closest edge (DropZone { edge }): data-edge follows the pointer live within
// the hovered zone, restricted to the allowed edge set, and the drop outcome
// delivers the edge held at release. The attribute leaves with the drag.
test("data-edge tracks the pointer and the drop carries the edge", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Closest edge");
  await sec.scrollIntoViewIfNeeded();
  const zone = sec.locator(".edge-zone");
  const status = sec.locator("#edge-status");

  const from = await sec.locator("#edge-drag").boundingBox();
  const to = await zone.boundingBox();
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  // Upper half reads top - even at the far left, where an unrestricted
  // nearest-of-four would say "left".
  await page.mouse.move(to.x + 10, to.y + to.height * 0.25, { steps: 15 });
  await expect(zone).toHaveAttribute("data-edge", "top");
  // Crossing the midline flips it to bottom, live.
  await page.mouse.move(to.x + to.width / 2, to.y + to.height * 0.8, { steps: 8 });
  await expect(zone).toHaveAttribute("data-edge", "bottom");
  await page.mouse.up();

  // The handler received the edge held at release, and the attribute left
  // with the drag.
  await expect(status).toHaveAttribute("data-landed", "edge:bottom");
  await expect(zone).not.toHaveAttribute("data-edge", /.+/);
});

// Localized voice (DndStrings): the keyboard announcements read the provided
// context, and a live locale switch changes the very next phrase - the
// components capture the struct once but the closures read the locale.
test("keyboard announcements speak the provided DndStrings locale", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Localized voice");
  await sec.scrollIntoViewIfNeeded();
  const voice = sec.locator('[role="status"]');
  const drag = sec.locator("#voice-drag");

  // English pickup, arrow to the shelf, drop.
  await drag.focus();
  await page.keyboard.press("Enter");
  await expect(voice).toHaveText("Picked up parcel. Use arrow keys, Enter to drop.");
  await page.keyboard.press("ArrowDown");
  await expect(voice).toHaveText("Over shelf.");
  await page.keyboard.press("Enter");
  await expect(voice).toHaveText("Dropped in shelf.");

  // Switch to Spanish mid-session: the next drag speaks it.
  await sec.locator("#voice-toggle").click();
  await drag.focus();
  await page.keyboard.press("Enter");
  await expect(voice).toHaveText("Recogiste parcel. Usa las flechas, Enter para soltar.");
  await page.keyboard.press("Escape");
  await expect(voice).toHaveText("Arrastre cancelado.");
});

// Virtualized list: rows mount and unmount as the window scrolls. A row that
// mounts MID-DRAG (recycled in by a wheel scroll while the payload is in
// flight) missed both the pickup measurement and the last scroll ping - it
// must still take the drop, because zones measure themselves on mount.
test("drops land on virtual rows recycled in mid-drag", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Virtual list");
  await sec.scrollIntoViewIfNeeded();
  const scroll = sec.locator(".virtual-scroll");
  const status = sec.locator("#virtual-status");

  const from = await sec.locator("#virtual-drag").boundingBox();
  const box = await scroll.boundingBox();

  // Pick the tag up and enter the list.
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 15 });

  // Scroll deep into the list mid-drag (wheel or scroll-to-index): the
  // entire visible window is now rows that did not exist at pickup. The
  // rows crossing the container's clip fire onvisible, which recovers the
  // offset - scroll events never reach dioxus-web, and pointer events
  // retarget to the captured drag source.
  await scroll.evaluate((node) => {
    node.scrollTop = 3000;
  });
  await expect(status).toHaveAttribute("data-window", /^9[0-9]\.\./);

  // Hover a freshly mounted row (a nudge so hit-testing re-runs against
  // its mount-time measurement), then drop.
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2 + 2);
  await page.waitForTimeout(100);
  const midRow = Math.floor((3000 + box.height / 2) / 30);
  await expect(scroll.getByText(`Row ${midRow}`, { exact: true })).toBeVisible();
  await page.mouse.up();

  // The drop landed on the recycled row under the pointer.
  await expect(status).toHaveAttribute("data-landed", `row:${midRow}:tag`);
});

// Keyboard drags reach virtual rows too: only the mounted window is
// registered, and arrows walk it in spatial order.
test("keyboard drop lands on a mounted virtual row", async ({ page }) => {
  await openFixtures(page);

  const sec = await section(page, "Virtual list");
  await sec.scrollIntoViewIfNeeded();
  const status = sec.locator("#virtual-status");
  const voice = sec.locator('[role="status"]');

  await sec.locator("#virtual-drag").focus();
  await page.keyboard.press("Enter");
  await page.keyboard.press("ArrowDown");
  await expect(voice).toHaveText("Over Row 0.");
  await page.keyboard.press("ArrowDown");
  await expect(voice).toHaveText("Over Row 1.");
  await page.keyboard.press("Enter");
  await expect(status).toHaveAttribute("data-landed", "row:1:tag");
});
