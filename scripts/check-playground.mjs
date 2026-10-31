// Checks the browser playground in headless Chromium, through the DevTools protocol: New saves a map, Open copies a
// picked file in without overwriting a kept one, and a full storage says so. Exits 1 on the first failure.
// Run after `sh scripts/web.sh`, from the repo: `node scripts/check-playground.mjs`. Needs Node 22+ and Chromium
// (`CHROMIUM=/path/to/chrome` picks another one).
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { extname, join } from "node:path";
import { tmpdir } from "node:os";

const site = new URL("../site/", import.meta.url).pathname;
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm", ".css": "text/css", ".png": "image/png" };
const server = createServer((req, res) => {
  const path = join(site, decodeURIComponent(new URL(req.url, "http://x").pathname).replace(/\/$/, "/index.html"));
  try {
    res.writeHead(200, { "Content-Type": types[extname(path)] ?? "application/octet-stream" }).end(readFileSync(path));
  } catch {
    res.writeHead(404).end();
  }
}).listen(0);
const temp = mkdtempSync(join(tmpdir(), "mindme-check-"));
const picked = join(temp, "check.md");
writeFileSync(picked, "# Go\n> A language from Google.\n- Goroutines\n");
// Bigger than what the filled storage has left.
const big = join(temp, "big.md");
writeFileSync(big, "# Big\n" + "- branch\n".repeat(8000));

const chrome = spawn(process.env.CHROMIUM ?? "chromium", ["--headless=new", "--no-sandbox", "--remote-debugging-port=9333", "--window-size=1280,800", `--user-data-dir=${temp}/profile`, "about:blank"], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const done = (code, message) => {
  console.log(message);
  chrome.kill();
  server.close();
  process.exit(code);
};
const check = (ok, what) => ok || done(1, `FAIL: ${what}`);

let page;
for (let i = 0; i < 50 && !page; i++) {
  await sleep(200);
  page = await fetch("http://127.0.0.1:9333/json").then((r) => r.json()).then((t) => t.find((p) => p.type === "page")).catch(() => null);
}
check(page, "Chromium did not start");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const replies = new Map();
const events = [];
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id) replies.get(d.id)?.(d.result ?? {});
  else events.push(d);
};
const send = (method, params = {}) => new Promise((r) => (replies.set(++id, r), ws.send(JSON.stringify({ id, method, params }))));
const run = async (expression) => (await send("Runtime.evaluate", { expression, returnByValue: true })).result?.value;
const key = async (k, modifiers = 0) => {
  for (const type of ["keyDown", "keyUp"]) await send("Input.dispatchKeyEvent", { type, key: k, code: k.length === 1 ? `Key${k.toUpperCase()}` : k, windowsVirtualKeyCode: k.length === 1 ? k.toUpperCase().charCodeAt(0) : 13, modifiers });
};
const until = async (what, test) => {
  for (let i = 0; i < 50; i++) {
    if (await test()) return;
    await sleep(200);
  }
  const shot = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(temp, "failed.png"), Buffer.from(shot.data, "base64"));
  check(false, `${what} (screenshot: ${join(temp, "failed.png")})`);
};
// Ctrl+O, then the browser's file chooser, answered with `file`.
const open = async (file) => {
  events.length = 0;
  await key("o", 2);
  await until("the file chooser never opened", async () => events.some((e) => e.method === "Page.fileChooserOpened"));
  const chooser = events.find((e) => e.method === "Page.fileChooserOpened").params;
  await send("DOM.setFileInputFiles", { backendNodeId: chooser.backendNodeId, files: [file] });
};

await send("Page.enable");
await send("Page.setInterceptFileChooserDialog", { enabled: true });
const { port } = server.address();
await send("Page.navigate", { url: `http://127.0.0.1:${port}/try/` });
await until("the page did not load", async () => (await run("document.readyState")) === "complete");
await run(`localStorage.setItem("map:/check.md", "# Kept\\n"); 1`);
await send("Page.reload");
await until("the playground did not start", async () => (await run(`!document.getElementById("loading")`)) === true);
await sleep(1500);
// A click gives the canvas the keyboard.
for (const type of ["mousePressed", "mouseReleased"]) await send("Input.dispatchMouseEvent", { type, x: 640, y: 400, button: "left", clickCount: 1 });

await key("n", 2);
await sleep(500);
await send("Input.insertText", { text: "Check topic" });
await key("Enter");
await until("New did not save a map as untitled.md", async () => (await run(`localStorage.getItem("map:/untitled.md")`))?.startsWith("# Check topic"));

await open(picked);
await until("Open did not copy the file in as check-2.md", async () => (await run(`localStorage.getItem("map:/check-2.md")`)) === readFileSync(picked, "utf8"));
check((await run(`localStorage.getItem("map:/check.md")`)) === "# Kept\n", "Open overwrote the kept check.md");

// Fill the storage, then open again: the browser says why nothing opened.
await run(`(() => { let s = "x".repeat(1 << 20), n = 0; try { for (;;) localStorage.setItem("fill" + n++, s); } catch {} try { for (;;) localStorage.setItem("fill" + n++, "x".repeat(1 << 12)); } catch {} return n; })()`);
await open(big);
await until("a full storage showed no message", async () => events.some((e) => e.method === "Page.javascriptDialogOpening" && e.params.message.includes("could not be opened")));
await send("Page.handleJavaScriptDialog", { accept: true });

done(0, "OK: New saves, Open copies without overwriting, a full storage says so.");
