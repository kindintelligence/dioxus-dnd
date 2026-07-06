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
