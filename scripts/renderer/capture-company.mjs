// Optional trusted local renderer. Requests arrive on stdin, never in argv or URLs.
// It renders the shipped frontend in a fresh browser; no model or scheduler runs here.

const views = new Set(["overview", "operations", "growth", "simulation", "reports", "files", "chronicle", "plugins"]);
let browser;
let watchdog;
try {
  const { chromium } = await import("playwright").catch(() => import("../../web/node_modules/playwright/index.mjs"));
  const chunks = [];
  let size = 0;
  for await (const chunk of process.stdin) {
    size += chunk.length;
    if (size > 16_384) throw new Error("request_too_large");
    chunks.push(chunk);
  }
  const input = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  const origin = new URL(input.base_url);
  if (!["http:", "https:"].includes(origin.protocol) || origin.username || origin.password || origin.pathname !== "/" || origin.search || origin.hash
      || !/^[a-zA-Z0-9_-]{1,128}$/.test(input.company_id) || !views.has(input.view)
      || !["en", "zh-CN"].includes(input.language) || typeof input.token !== "string" || !input.token.startsWith("tpc_exp_")) {
    throw new Error("invalid_request");
  }
  watchdog = setTimeout(() => { void browser?.close(); }, 30_000);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 960 }, locale: input.language, deviceScaleFactor: 1 });
  await context.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin.origin && !["data:", "blob:"].includes(url.protocol)) await route.abort();
    else await route.continue();
  });
  await context.addInitScript(({ token, language }) => {
    sessionStorage.setItem("transplane-company-credential", token);
    localStorage.setItem("transplane-company-locale", language);
  }, { token: input.token, language: input.language });
  const page = await context.newPage();
  const pending = new Set();
  const important = (request) => new URL(request.url()).pathname.startsWith("/api/") && !new URL(request.url()).pathname.endsWith("/changes");
  page.on("request", (request) => { if (important(request)) pending.add(request); });
  page.on("requestfinished", (request) => pending.delete(request));
  page.on("requestfailed", (request) => pending.delete(request));
  const url = new URL(`?company=${encodeURIComponent(input.company_id)}#${input.view}`, origin);
  await page.goto(url.href, { waitUntil: "domcontentloaded", timeout: 15_000 });
  const main = page.locator("#main-content");
  await page.waitForFunction(({ companyId, view }) => {
    const element = document.getElementById("main-content");
    return element?.dataset.companyId === companyId && element.dataset.companyView === view && element.dataset.viewReady === "true";
  }, { companyId: input.company_id, view: input.view }, { timeout: 15_000 });
  await page.evaluate(() => document.fonts.ready);
  await page.waitForFunction(() => [...document.querySelectorAll("#main-content img")].every((image) => image.complete && image.naturalWidth > 0), undefined, { timeout: 5_000 });
  await page.evaluate(() => Promise.all([...document.querySelectorAll("#main-content img")].map((image) => image.decode())));
  let output;
  for (let attempt = 0; attempt < 3; attempt++) {
    // Submodules may load records after the main snapshot. Wait for those reads
    // without waiting for the deliberately open company-change request.
    let quietSince = Date.now();
    const until = Date.now() + 5_000;
    while (Date.now() < until) {
      if (pending.size) quietSince = Date.now();
      else if (Date.now() - quietSince >= 350) break;
      await page.waitForTimeout(50);
    }
    if (pending.size) throw new Error("module_not_ready");
    await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const before = await main.getAttribute("data-company-revision");
    if (!Number.isSafeInteger(Number(before)) || Number(before) < 1) throw new Error("revision_unavailable");
    const changeUrl = new URL(`/api/v1/companies/${encodeURIComponent(input.company_id)}/changes?after_revision=${before}&wait_ms=0`, origin);
    const checkVersion = async () => {
      const response = await context.request.get(changeUrl.href, { headers: { Authorization: `Bearer ${input.token}` }, timeout: 3_000 });
      if (!response.ok()) throw new Error("company_read_failed");
      return !(await response.json()).changed;
    };
    if (!await checkVersion()) { await page.waitForTimeout(500); continue; }
    const bounds = await main.boundingBox();
    if (!bounds || bounds.width > 2048 || bounds.height > 8192) throw new Error("module_too_large");
    const bytes = await main.screenshot({ type: "png", animations: "disabled", timeout: 5_000 });
    const after = await main.getAttribute("data-company-revision");
    if (before !== after || await main.getAttribute("data-view-ready") !== "true" || !await checkVersion()) continue;
    if (bytes.length > 8 * 1024 * 1024) throw new Error("image_too_large");
    output = { company_id: input.company_id, view: input.view, data_revision: Number(before), image: { mime_type: "image/png", base64: bytes.toString("base64") } };
    break;
  }
  if (!output) throw new Error("state_changed_during_capture");
  process.stdout.write(JSON.stringify(output));
} catch {
  // Deliberately omit exception text: browser errors can include URLs or secrets.
  process.stdout.write(JSON.stringify({ error: { code: "screenshot_failed" } }));
  process.exitCode = 1;
} finally {
  clearTimeout(watchdog);
  await browser?.close();
}
