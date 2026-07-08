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

async function openGallery(page) {
  await page.goto("/dioxus-dnd/", { waitUntil: "domcontentloaded" });
  await page.addStyleTag({
    content: '[id^="__dx-toast"], .dx-toast { display: none !important; pointer-events: none !important; }',
  });
  await expect(page.getByRole("heading", { name: "Drag & drop gallery" })).toBeVisible({
    timeout: 60_000,
  });
  await expect
    .poll(
      async () => {
        const sortable = await section(page, "Sortable list");
        const box = await elementBox(sortable, "Research");
        return box ? Math.round(box.width) : 0;
      },
      { timeout: 60_000 },
    )
    .toBeLessThan(1100);
}

async function openCanvasExample(page) {
  await page.goto("http://127.0.0.1:8081/dioxus-dnd/", { waitUntil: "domcontentloaded" });
  await page.addStyleTag({
    content: '[id^="__dx-toast"], .dx-toast { display: none !important; pointer-events: none !important; }',
  });
  await expect(page.getByRole("heading", { name: "Workflow canvas" })).toBeVisible({
    timeout: 60_000,
  });
}

async function openShowcaseSortable(page) {
  await page.goto("http://127.0.0.1:8082/dioxus-dnd/#demo-2", {
    waitUntil: "domcontentloaded",
  });
  await page.addStyleTag({
    content: '[id^="__dx-toast"], .dx-toast { display: none !important; pointer-events: none !important; }',
  });
  await expect(page.getByRole("heading", { name: "PICK, DROP & SHIP" })).toBeVisible({
    timeout: 60_000,
  });
  const demo = page.locator("#demo-2");
  await expect(demo.getByRole("heading", { name: "Sort a list" })).toBeVisible({
    timeout: 60_000,
  });
  return demo;
}

async function canvasNodeBox(canvas, text) {
  return canvas.evaluate((root, text) => {
    const node = Array.from(root.children).find((child) => {
      const style = window.getComputedStyle(child);
      return style.position === "absolute" && child.textContent.includes(text);
    });
    if (!node) {
      return null;
    }
    const rect = node.getBoundingClientRect();
    const style = window.getComputedStyle(node);
    return {
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height,
      left: Number.parseFloat(style.left),
      top: Number.parseFloat(style.top),
      worldLeft: Number.parseFloat(node.dataset.worldX),
      worldTop: Number.parseFloat(node.dataset.worldY),
      worldWidth: Number.parseFloat(node.dataset.worldWidth),
      worldHeight: Number.parseFloat(node.dataset.worldHeight),
    };
  }, text);
}

async function latestCanvasNodeBox(canvas, text) {
  return canvas.evaluate((root, text) => {
    const nodes = Array.from(root.children).filter((child) => {
      const style = window.getComputedStyle(child);
      return style.position === "absolute" && child.textContent.includes(text);
    });
    const node = nodes.at(-1);
    if (!node) {
      return null;
    }
    return {
      worldLeft: Number.parseFloat(node.dataset.worldX),
      worldTop: Number.parseFloat(node.dataset.worldY),
    };
  }, text);
}

async function keyboardPreview(page) {
  return page
    .locator("section", { has: page.getByRole("heading", { name: "Builder" }) })
    .locator('[data-keyboard-placement-preview="true"]')
    .evaluate((node) => {
      const style = window.getComputedStyle(node);
      return {
        left: Number.parseFloat(style.left),
        top: Number.parseFloat(style.top),
        label: node.textContent.trim(),
      };
    });
}

async function keyboardCreateFromPalette(page, label) {
  const paletteNode = page
    .locator("aside", { has: page.getByRole("heading", { name: "Blocks" }) })
    .getByText(label, { exact: true });
  const canvas = page
    .locator("section", { has: page.getByRole("heading", { name: "Builder" }) })
    .locator(".relative")
    .first();

  await paletteNode.focus();
  await page.keyboard.press(" ");
  await expect(canvas).toHaveAttribute("data-active", "true");
  await page.keyboard.press("Enter");
}

async function selectKeyboardPlacement(page, label) {
  const button = page.getByRole("button", { name: `Keyboard placement ${label}` });
  await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "true");
}

async function openRegressions(page) {
  await page.goto("http://127.0.0.1:8083/dioxus-dnd/", { waitUntil: "domcontentloaded" });
  await expect(page.getByRole("heading", { name: "Regressions" })).toBeVisible({ timeout: 60_000 });
}

async function dispatchNativeDropAt(target, setup, clientX, clientY) {
  return target.evaluate(
    (node, { setup, clientX, clientY }) => {
      const dataTransfer = new DataTransfer();
      for (const [type, value] of Object.entries(setup.data || {})) {
        dataTransfer.setData(type, value);
      }
      const init = { bubbles: true, cancelable: true, clientX, clientY, dataTransfer };
      for (const type of ["dragenter", "dragover", "drop"]) {
        node.dispatchEvent(new DragEvent(type, init));
      }
    },
    { setup, clientX, clientY },
  );
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
  await openGallery(page);

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
  await expect(sortable.locator('[data-dragging]').first()).toBeVisible();
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

test("autoscroll follows default mouse pointer drags near the edge", async ({ page }) => {
  const demo = await openShowcaseSortable(page);
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

  await page.mouse.move(edgeX, edgeY);
  await page.waitForTimeout(150);
  expect(await scroll.evaluate((node) => node.scrollTop)).toBe(0);

  const handle = scroll.locator("[data-sort-handle]").first();
  const handleBox = await handle.boundingBox();
  expect(handleBox).not.toBeNull();

  await page.mouse.move(handleBox.x + handleBox.width / 2, handleBox.y + handleBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(handleBox.x + handleBox.width / 2 + 24, handleBox.y + handleBox.height / 2 + 24, {
    steps: 5,
  });
  await expect(scroll.locator('[data-dragging="true"]').filter({ hasText: "Unload the truck" }).first()).toBeVisible();

  for (let i = 0; i < 12; i += 1) {
    await page.mouse.move(edgeX, edgeY - (i % 2), { steps: 2 });
    await page.waitForTimeout(25);
  }

  await expect
    .poll(async () => scroll.evaluate((node) => node.scrollTop), { timeout: 5_000 })
    .toBeGreaterThan(0);

  await page.mouse.up();
});

test("canvas pointer drop uses the recorded grab offset", async ({ page }) => {
  await openGallery(page);

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
  await expect(canvas.locator('[data-dragging="true"]').filter({ hasText: "Input" }).first()).toBeVisible();
  await page.mouse.move(endX, endY, { steps: 30 });
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.waitForTimeout(300);

  const after = await node.boundingBox();
  expect(after).not.toBeNull();
  expect(Math.abs(after.x - before.x)).toBeGreaterThan(40);
  expect(Math.abs(after.y - before.y)).toBeGreaterThan(30);
});

test("focused canvas example creates, connects, and keeps nodes inside bounds", async ({ page }) => {
  await openCanvasExample(page);

  const builder = page.locator("section", { has: page.getByRole("heading", { name: "Builder" }) });
  const canvas = builder.locator(".relative").first();
  const inspector = page.locator("aside", { has: page.getByRole("heading", { name: "Inspector" }) });
  const canvasBox = await canvas.boundingBox();
  expect(canvasBox).not.toBeNull();

  await page.getByLabel("Connect from Bad").click();
  await page.getByLabel("Connect into Publish Results").click();
  await expect(page.getByText("Connected node 1 to node 3.", { exact: true })).toBeVisible();
  await expect(inspector.locator("div", { hasText: "Connections" }).getByText("3", { exact: true })).toBeVisible();

  const before = await canvasNodeBox(canvas, "Find Comparable Products");
  expect(before).not.toBeNull();

  await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
  await page.mouse.down();
  await page.mouse.move(canvasBox.x + canvasBox.width - 8, canvasBox.y + canvasBox.height - 8, {
    steps: 36,
  });
  await page.mouse.up();
  await page.waitForTimeout(300);

  const after = await canvasNodeBox(canvas, "Find Comparable Products");
  expect(after).not.toBeNull();
  expect(after.left).toBeGreaterThanOrEqual(0);
  expect(after.top).toBeGreaterThanOrEqual(0);
  expect(after.left + after.width).toBeLessThanOrEqual(canvasBox.width + 1);
  expect(after.top + after.height).toBeLessThanOrEqual(canvasBox.height + 1);

  const paletteNode = page
    .locator("aside", { has: page.getByRole("heading", { name: "Blocks" }) })
    .getByText("Publish Results", { exact: true });
  const paletteBox = await paletteNode.boundingBox();
  expect(paletteBox).not.toBeNull();
  const createCanvasBox = await canvas.boundingBox();
  expect(createCanvasBox).not.toBeNull();

  await page.mouse.move(paletteBox.x + paletteBox.width / 2, paletteBox.y + paletteBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(paletteBox.x + paletteBox.width / 2 + 20, paletteBox.y + paletteBox.height / 2 + 20, {
    steps: 5,
  });
  await page.mouse.move(createCanvasBox.x + 96, createCanvasBox.y + createCanvasBox.height - 48, {
    steps: 28,
  });
  await page.mouse.up();
  await page.waitForTimeout(300);

  await expect(page.getByText(/^Created Publish Results at /)).toBeVisible();
  await expect(inspector.locator("div", { hasText: "Nodes" }).getByText("4", { exact: true })).toBeVisible();
});

test("focused canvas example keeps its coordinate plane scrollable on mobile", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await openCanvasExample(page);

  const builder = page.locator("section", { has: page.getByRole("heading", { name: "Builder" }) });
  const canvas = builder.locator(".relative").first();
  const canvasBox = await canvas.boundingBox();
  expect(canvasBox).not.toBeNull();
  expect(Math.round(canvasBox.width)).toBe(960);

  for (const label of ["Bad", "Find Comparable Products", "Publish Results"]) {
    const node = await canvasNodeBox(canvas, label);
    expect(node).not.toBeNull();
    expect(node.left).toBeGreaterThanOrEqual(0);
    expect(node.top).toBeGreaterThanOrEqual(0);
    expect(node.left + node.width).toBeLessThanOrEqual(canvasBox.width + 1);
    expect(node.top + node.height).toBeLessThanOrEqual(canvasBox.height + 1);
  }
});

test("focused canvas example moves nodes after zoom and pan", async ({ page }) => {
  await openCanvasExample(page);

  const builder = page.locator("section", { has: page.getByRole("heading", { name: "Builder" }) });
  const canvas = builder.locator(".relative").first();

  await page.getByLabel("Zoom canvas in").click();
  await page.getByLabel("Pan canvas left").click();

  const canvasBox = await canvas.boundingBox();
  expect(canvasBox).not.toBeNull();

  const before = await canvasNodeBox(canvas, "Find Comparable Products");
  expect(before).not.toBeNull();

  let after = before;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const current = await canvasNodeBox(canvas, "Find Comparable Products");
    expect(current).not.toBeNull();

    await page.mouse.move(current.x + current.width / 2, current.y + current.height / 2);
    await page.mouse.down();
    await page.mouse.move(current.x + current.width / 2 + 24, current.y + current.height / 2 + 24, {
      steps: 5,
    });
    await expect(canvas.locator('[data-dragging="true"]').filter({ hasText: "Find Comparable Products" }).first()).toBeVisible();
    await page.mouse.move(canvasBox.x + canvasBox.width * 0.75, canvasBox.y + canvasBox.height * 0.72, {
      steps: 36,
    });
    await page.mouse.up();
    await page.waitForTimeout(300);

    after = await canvasNodeBox(canvas, "Find Comparable Products");
    expect(after).not.toBeNull();
    if (after.worldLeft !== before.worldLeft || after.worldTop !== before.worldTop) {
      break;
    }
  }

  expect(after).not.toBeNull();
  expect(after.worldLeft).not.toBe(before.worldLeft);
  expect(after.worldTop).not.toBe(before.worldTop);
  expect(after.worldLeft).toBeGreaterThanOrEqual(0);
  expect(after.worldTop).toBeGreaterThanOrEqual(0);
  expect(after.worldLeft + after.worldWidth).toBeLessThanOrEqual(960 + 1);
  expect(after.worldTop + after.worldHeight).toBeLessThanOrEqual(560 + 1);
});

test("focused canvas keyboard placement policies use the selected toolbar policy", async ({ page }) => {
  await openCanvasExample(page);

  const canvas = page
    .locator("section", { has: page.getByRole("heading", { name: "Builder" }) })
    .locator(".relative")
    .first();

  expect(await keyboardPreview(page)).toEqual({ left: 480, top: 280, label: "Center" });
  await keyboardCreateFromPalette(page, "Bad");
  await expect(page.getByText("Created Bad at (480, 288)", { exact: true })).toBeVisible();
  await expect.poll(async () => latestCanvasNodeBox(canvas, "Bad")).toEqual({
    worldLeft: 480,
    worldTop: 288,
  });

  await selectKeyboardPlacement(page, "Origin");
  expect(await keyboardPreview(page)).toEqual({ left: 0, top: 0, label: "Origin" });
  await keyboardCreateFromPalette(page, "Find Comparable Products");
  await expect(page.getByText("Created Find Comparable Products at (0, 0)", { exact: true })).toBeVisible();
  await expect.poll(async () => latestCanvasNodeBox(canvas, "Find Comparable Products")).toEqual({
    worldLeft: 0,
    worldTop: 0,
  });

  await selectKeyboardPlacement(page, "Fixed");
  expect(await keyboardPreview(page)).toEqual({ left: 744, top: 408, label: "Fixed" });
  await keyboardCreateFromPalette(page, "Publish Results");
  await expect(page.getByText("Created Publish Results at (744, 408)", { exact: true })).toBeVisible();
  await expect.poll(async () => latestCanvasNodeBox(canvas, "Publish Results")).toEqual({
    worldLeft: 744,
    worldTop: 408,
  });
});

test("focused canvas native boundary surface accepts DataTransfer drops", async ({ page }) => {
  await openCanvasExample(page);

  const boundary = page.locator("section", {
    has: page.getByRole("heading", { name: "Native boundary" }),
  });
  const zone = boundary.locator(".relative").first();

  await dispatchNativeDrop(zone, {
    data: {
      "text/uri-list": "https://dioxuslabs.com\n",
      "text/plain": "https://dioxuslabs.com",
    },
  });
  await expect(boundary.getByText("Link https://dioxuslabs.com", { exact: true })).toBeVisible();

  await dispatchNativeDrop(zone, {
    file: { name: "requirements.txt", type: "text/plain", body: "native canvas note" },
  });
  await expect(boundary.getByText("File requirements.txt", { exact: true })).toBeVisible();
  await expect(page.getByText(/^Native drop: File requirements.txt at/)).toBeVisible();
});

test("native DataTransfer paths handle files, external drops, and drag-out", async ({ page }) => {
  await openGallery(page);

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

  const outbound = await inOut.getByText(/^Drag this link out/).evaluate((node) => {
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

// A pointer drag released outside the list/grid must commit no reorder, the
// same way the native path cancels a drop that lands off the rows. (Regression
// for the pointer path previously snapping to the last-hovered target.)
test("sortable release outside the list commits no reorder", async ({ page }) => {
  await openGallery(page);

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
  await openGallery(page);

  const sortable = await section(page, "Sortable list");
  await sortable.scrollIntoViewIfNeeded();
  const source = await elementBox(sortable, "Research");
  const target = await elementBox(sortable, "Revise");
  expect(source).not.toBeNull();
  expect(target).not.toBeNull();

  await page.mouse.move(source.x + source.width / 2, source.y + source.height / 2);
  await page.mouse.down();
  await page.mouse.move(source.x + source.width / 2, source.y + source.height * 1.6, { steps: 6 });
  await expect(sortable.locator('[data-dragging]').first()).toBeVisible();
  await page.mouse.move(source.x + source.width / 2, target.y + target.height * 0.75, { steps: 24 });
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.waitForTimeout(300);

  // Research moved down past Revise.
  await expect
    .poll(() => sortableRowTexts(page))
    .toEqual(["Draft", "Review", "Revise", "Research", "Publish"]);
});

test("grid release outside the tiles commits no reorder", async ({ page }) => {
  await openGallery(page);

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

// Autoscroll must not run away: once the pointer leaves the container, scrolling
// stops even though (under pointer capture) move events keep bubbling in.
test("autoscroll stops when the pointer leaves the container", async ({ page }) => {
  const demo = await openShowcaseSortable(page);
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

// A pointer drop over a zone that rejects the payload must fall through to an
// accepting zone stacked underneath, not cancel. (Regression for finish_drop
// cancelling when the geometric-topmost zone rejected.)
test("pointer drop falls through a rejecting zone to the accepting one under it", async ({ page }) => {
  await openRegressions(page);

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
// the same as the native path - so a Ctrl-drag leaves the source in place.
test("ctrl-drag on the pointer path copies instead of moving", async ({ page }) => {
  await openGallery(page);

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

// ReorderButtons rendered inside a SortableList row must still receive clicks -
// pressing one must not let the row grab pointer capture and swallow the click.
test("reorder buttons reorder from inside a sortable row", async ({ page }) => {
  await openGallery(page);

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

// A native drop landing on a child node inside a canvas must report
// canvas-relative coordinates, not coordinates relative to the child element.
test("canvas native drop over a child reports canvas-relative coordinates", async ({ page }) => {
  await openRegressions(page);

  const canvas = page.locator("#canvas-child").locator("xpath=ancestor::div[1]");
  const child = page.locator("#canvas-child");
  const out = page.locator("#canvas-drop-pointer");

  const canvasBox = await canvas.boundingBox();
  const childBox = await child.boundingBox();
  expect(canvasBox).not.toBeNull();
  expect(childBox).not.toBeNull();

  // Where we'll drop: the centre of the child (which sits at ~200,120 inside
  // the canvas). In canvas coordinates that is child-centre minus canvas origin.
  const dropX = childBox.x + childBox.width / 2;
  const dropY = childBox.y + childBox.height / 2;
  const expectedX = dropX - canvasBox.x;
  const expectedY = dropY - canvasBox.y;

  // Start a native HTML5 drag on the source, then drop onto the child.
  const source = page.getByText("native source", { exact: true });
  await source.evaluate((node) => {
    node.dispatchEvent(
      new DragEvent("dragstart", { bubbles: true, cancelable: true, dataTransfer: new DataTransfer() }),
    );
  });
  await dispatchNativeDropAt(child, { data: { "text/plain": "x" } }, dropX, dropY);

  await expect(out).toHaveAttribute("data-set", "true", { timeout: 5_000 });
  const got = await out.evaluate((n) => ({
    x: Number.parseFloat(n.dataset.x),
    y: Number.parseFloat(n.dataset.y),
  }));

  // Canvas-relative (~240, ~135), NOT child-relative (~40, ~15).
  expect(Math.abs(got.x - expectedX)).toBeLessThan(2);
  expect(Math.abs(got.y - expectedY)).toBeLessThan(2);
  expect(got.x).toBeGreaterThan(150);
  expect(got.y).toBeGreaterThan(90);
});
