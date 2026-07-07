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
  expect(Math.round(canvasBox.width)).toBe(720);

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

  await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
  await page.mouse.down();
  await page.mouse.move(before.x + before.width / 2 + 24, before.y + before.height / 2 + 24, {
    steps: 5,
  });
  await expect(canvas.locator('[data-dragging="true"]').filter({ hasText: "Find Comparable Products" }).first()).toBeVisible();
  await page.mouse.move(canvasBox.x + canvasBox.width - 8, canvasBox.y + canvasBox.height - 8, {
    steps: 36,
  });
  await page.mouse.up();
  await page.waitForTimeout(300);

  const after = await canvasNodeBox(canvas, "Find Comparable Products");
  expect(after).not.toBeNull();
  expect(after.worldLeft).not.toBe(before.worldLeft);
  expect(after.worldTop).not.toBe(before.worldTop);
  expect(after.worldLeft).toBeGreaterThanOrEqual(0);
  expect(after.worldTop).toBeGreaterThanOrEqual(0);
  expect(after.worldLeft + after.worldWidth).toBeLessThanOrEqual(720 + 1);
  expect(after.worldTop + after.worldHeight).toBeLessThanOrEqual(420 + 1);
});

test("focused canvas keyboard drop lands at the selected canvas geometry", async ({ page }) => {
  await openCanvasExample(page);

  const paletteNode = page
    .locator("aside", { has: page.getByRole("heading", { name: "Blocks" }) })
    .getByText("Bad", { exact: true });

  await paletteNode.focus();
  await page.keyboard.press(" ");
  await page.keyboard.press("Enter");

  await expect(page.getByText("Created Bad at (360, 216)", { exact: true })).toBeVisible();
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
