// Typed DataTransfer transport (external::typed + TypedDragSource /
// TypedDropZone), driven with REAL DragEvents carrying a REAL DataTransfer
// in a real engine - the boundary the headless suite cannot cross.
// Synthetic mouse moves don't start HTML5 drags reliably in automation, so
// the drag lifecycle is dispatched directly; a constructed DataTransfer is
// not protected-mode-gated, which also lets the spec inspect what the
// source wrote.
const { test, expect } = require("@playwright/test");

async function openFixtures(page) {
  await page.goto("/dioxus-dnd/", { waitUntil: "domcontentloaded" });
  await expect(page.getByRole("heading", { name: "Regressions" })).toBeVisible({
    timeout: 60_000,
  });
  await page
    .locator("section", { has: page.getByRole("heading", { name: "Typed transport" }) })
    .scrollIntoViewIfNeeded();
}

// Dispatch a full dragstart -> dragenter -> dragover -> drop arc from one
// selector to another through a single DataTransfer, returning what the
// source wrote onto it.
async function dragArc(page, sourceSel, zoneSel) {
  return page.evaluate(
    ([sourceSel, zoneSel]) => {
      const dt = new DataTransfer();
      const source = document.querySelector(sourceSel);
      const zone = document.querySelector(zoneSel);
      const fire = (el, type) =>
        el.dispatchEvent(
          new DragEvent(type, {
            dataTransfer: dt,
            bubbles: true,
            cancelable: true,
            clientX: 10,
            clientY: 20,
          }),
        );
      fire(source, "dragstart");
      const written = {
        json: dt.getData("application/json"),
        text: dt.getData("text/plain"),
      };
      fire(zone, "dragenter");
      fire(zone, "dragover");
      fire(zone, "drop");
      return written;
    },
    [sourceSel, zoneSel],
  );
}

test("typed payload round-trips source to zone through a real DataTransfer", async ({ page }) => {
  await openFixtures(page);
  const status = page.locator("#typed-status");

  const written = await dragArc(page, "#typed-source", "#typed-zone");
  // The source wrote both representations: typed JSON plus a legible
  // text/plain fallback (defaulting to the JSON itself).
  expect(JSON.parse(written.json)).toEqual({ id: 7, name: "seven" });
  expect(written.text).toBe(written.json);

  // The zone decoded the payload back to the Rust type.
  await expect(status).toHaveAttribute("data-landed", "7:seven");
  await expect(status).toHaveAttribute("data-invalid", "0");
});

test("explicit fallback_text replaces the JSON fallback", async ({ page }) => {
  await openFixtures(page);

  const written = await dragArc(page, "#typed-source-fallback", "#typed-zone");
  expect(JSON.parse(written.json)).toEqual({ id: 9, name: "nine" });
  expect(written.text).toBe("card nine");
  await expect(page.locator("#typed-status")).toHaveAttribute("data-landed", "9:nine");
});

test("zone highlights on hover and ignores or reports foreign payloads", async ({ page }) => {
  await openFixtures(page);
  const zone = page.locator("#typed-zone");
  const status = page.locator("#typed-status");

  // Hover contract: data-over during a drag over the zone, gone on leave.
  await page.evaluate(() => {
    const dt = new DataTransfer();
    const zone = document.querySelector("#typed-zone");
    zone.dispatchEvent(
      new DragEvent("dragenter", { dataTransfer: dt, bubbles: true, cancelable: true }),
    );
  });
  await expect(zone).toHaveAttribute("data-over", "true");
  await page.evaluate(() => {
    const zone = document.querySelector("#typed-zone");
    zone.dispatchEvent(new DragEvent("dragleave", { bubbles: true }));
  });
  await expect(zone).not.toHaveAttribute("data-over", "true");

  // An untyped drag (text only): silently not ours.
  await page.evaluate(() => {
    const dt = new DataTransfer();
    dt.setData("text/plain", "just text");
    const zone = document.querySelector("#typed-zone");
    zone.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  await expect(status).toHaveAttribute("data-landed", "");
  await expect(status).toHaveAttribute("data-invalid", "0");

  // Typed-but-wrong JSON: reported through on_invalid, nothing lands.
  await page.evaluate(() => {
    const dt = new DataTransfer();
    dt.setData("application/json", '{"wrong": "shape"}');
    const zone = document.querySelector("#typed-zone");
    zone.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  await expect(status).toHaveAttribute("data-invalid", "1");
  await expect(status).toHaveAttribute("data-landed", "");
});
