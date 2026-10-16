import { test, expect } from "@playwright/test";
import { app, appUrl, GATEWAY_PORT, playMove } from "./helpers";

test("computer play supports click, keyboard, drag, undo and mute", async ({ page }) => {
  await page.addInitScript(() => {
    const Original = window.AudioContext;
    const audio = { contexts: [] as AudioContext[], tones: 0 };
    (window as any).freechessAudio = audio;
    window.AudioContext = class extends Original {
      constructor(options?: AudioContextOptions) {
        super(options);
        audio.contexts.push(this);
      }
      createOscillator() {
        audio.tones++;
        return super.createOscillator();
      }
    };
  });
  await page.goto(appUrl(GATEWAY_PORT));
  const frame = app(page);
  await frame.getByRole("button", { name: "Play computer", exact: true }).click();
  await expect(frame.locator(".board > .sq")).toHaveCount(64);
  await expect(frame.locator('.sq[data-square="e2"]')).toHaveAttribute("data-piece", "P");

  await playMove(frame, "e2", "e4");
  await expect(frame.locator(".moves .mv")).toHaveCount(2);
  await expect(frame.locator('.sq[data-square="e4"]')).toHaveAttribute("data-piece", "P");
  const content = await frame.locator("body").elementHandle();
  expect(await content!.evaluate(() => (window as any).freechessAudio.tones)).toBeGreaterThan(0);
  expect(await content!.evaluate(() => (window as any).freechessAudio.contexts[0].state)).toBe("running");

  await frame.getByRole("button", { name: "Undo turn", exact: true }).click();
  await expect(frame.locator(".moves .mv")).toHaveCount(0);
  await frame.locator('.sq[data-square="e2"]').focus();
  await frame.locator('.sq[data-square="e2"]').press("Enter");
  await frame.locator('.sq[data-square="e2"]').press("ArrowUp");
  await frame.locator('.sq[data-square="e3"]').press("ArrowUp");
  await frame.locator('.sq[data-square="e4"]').press("Enter");
  await expect(frame.locator(".moves .mv")).toHaveCount(2);

  await frame.getByRole("button", { name: "Mute sounds", exact: true }).click();
  const before = await content!.evaluate(() => (window as any).freechessAudio.tones);
  await frame.locator('.sq[data-square="g1"]').dragTo(frame.locator('.sq[data-square="f3"]'));
  await expect(frame.locator(".moves .mv")).toHaveCount(4);
  expect(await content!.evaluate(() => (window as any).freechessAudio.tones)).toBe(before);
  await frame.getByRole("button", { name: "Enable sounds", exact: true }).click();
  await frame.getByRole("button", { name: "New game", exact: true }).click();
  await expect(frame.locator(".moves .mv")).toHaveCount(0);
});

test("drag preview contains only transparent piece artwork and drops the legal move", async ({ page }) => {
  await page.addInitScript(() => {
    const previews: {
      onlyPieceArtwork: boolean;
      transparent: boolean;
      forbiddenElements: number;
      text: string;
    }[] = [];
    (window as any).freechessDragPreviews = previews;
    const snapshot = (element: Element) => {
      const pieces = [element, ...element.querySelectorAll("*")]
        .filter(node => node.matches("svg.piece-svg"));
      const piece = pieces[0];
      const nodes = [element, ...element.querySelectorAll("*")];
      return {
        onlyPieceArtwork: pieces.length === 1 && nodes.every(node =>
          node === element || node === piece || piece?.contains(node)),
        transparent: nodes.every(node => {
          const style = getComputedStyle(node);
          return style.backgroundColor === "rgba(0, 0, 0, 0)" && style.backgroundImage === "none";
        }),
        forbiddenElements: nodes.filter(node => node.matches(".sq, .coord, .hint")).length,
        text: element.textContent?.trim() ?? "",
      };
    };
    document.addEventListener("dragstart", event => {
      if (event.target instanceof Element && event.target.closest(".board")) {
        previews.push(snapshot(event.target));
      }
    }, true);
    const setDragImage = DataTransfer.prototype.setDragImage;
    DataTransfer.prototype.setDragImage = function (element, x, y) {
      // An explicit drag image overrides the native dragstart target's preview.
      if (previews.length) previews[previews.length - 1] = snapshot(element);
      return setDragImage.call(this, element, x, y);
    };
  });
  await page.goto(appUrl(GATEWAY_PORT));
  const frame = app(page);
  await frame.getByRole("button", { name: "Play computer", exact: true }).click();
  const source = frame.locator('.board > .sq[data-square="g1"]');
  const target = frame.locator('.board > .sq[data-square="f3"]');
  await expect(source).toHaveAttribute("data-piece", "N");
  const from = await source.boundingBox();
  const to = await target.boundingBox();
  expect(from).not.toBeNull();
  expect(to).not.toBeNull();

  await page.mouse.move(from!.x + from!.width / 2, from!.y + from!.height / 2);
  await page.mouse.down();
  try {
    await page.mouse.move(from!.x + from!.width / 2 + 12, from!.y + from!.height / 2 - 12, { steps: 4 });
    await expect(source).toHaveAttribute("data-piece", "N");
    await expect(source.locator("svg.piece-svg")).toHaveCount(1);
    const previews = await source.evaluate(() => (window as any).freechessDragPreviews);
    expect(previews, "A real native piece drag must start").toHaveLength(1);
    expect(previews[0]).toEqual({
      onlyPieceArtwork: true,
      transparent: true,
      forbiddenElements: 0,
      text: "",
    });
    await page.mouse.move(to!.x + to!.width / 2, to!.y + to!.height / 2, { steps: 8 });
  } finally {
    await page.mouse.up();
  }
  await expect(source).toHaveAttribute("data-piece", "");
  await expect(target).toHaveAttribute("data-piece", "N");
  await expect(frame.locator(".moves .mv")).toHaveCount(2);
});

test("mobile computer board fits and works without a connected node", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.routeWebSocket("**/v1/contract/command*", socket => socket.close());
  await page.goto(appUrl(GATEWAY_PORT));
  const frame = app(page);
  await frame.getByRole("button", { name: "Play computer", exact: true }).click();
  await playMove(frame, "e2", "e4");
  await expect(frame.locator(".moves .mv")).toHaveCount(2);
  await expect(frame.locator("#nickname-prompt")).toBeHidden();
  const body = await frame.locator("body").elementHandle();
  expect(await body!.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});
