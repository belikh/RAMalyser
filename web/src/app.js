// RAMalyser web interface. It speaks the tester's JSON console
// ("json list", "json basic 0", ...) through one of two links:
//   HttpLink    the page is served by the Wi-Fi module (api/command, api/state)
//   SerialLink  the page talks to the tester's USB serial port (Web Serial)
"use strict";

const $ = (id) => document.getElementById(id);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

class HttpLink {
  constructor() { this.name = "Wi-Fi module"; }

  async state() {
    const r = await fetch("api/state", { cache: "no-store" });
    if (!r.ok) throw new Error("the module answered " + r.status);
    return r.json();
  }

  async send(cmd) {
    const r = await fetch("api/command", { method: "POST", body: cmd });
    if (r.status === 409) throw new Error("the tester is busy with another test");
    if (!r.ok) throw new Error("the module answered " + r.status);
    for (;;) {
      await sleep(400);
      const s = await this.state();
      if (!s.busy) {
        if (!s.result) throw new Error("the tester did not answer");
        return s.result;
      }
    }
  }
}

class SerialLink {
  constructor(port) {
    this.name = "USB";
    this.port = port;
    this.pending = "";
    this.lines = [];
    this.waiting = null;
  }

  async open() {
    await this.port.open({ baudRate: 115200 });
    this.writer = this.port.writable.getWriter();
    this.pump();
  }

  async pump() {
    const reader = this.port.readable.pipeThrough(new TextDecoderStream()).getReader();
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      this.pending += value;
      let at;
      while ((at = this.pending.indexOf("\n")) >= 0) {
        const line = this.pending.slice(0, at).trim();
        this.pending = this.pending.slice(at + 1);
        if (line.startsWith("{")) {          // everything else is console text
          if (this.waiting) { this.waiting(line); this.waiting = null; } else this.lines.push(line);
        }
      }
    }
  }

  async send(cmd) {
    this.lines = [];
    await this.writer.write(new TextEncoder().encode("json " + cmd + "\n"));
    const line = await new Promise((resolve) => { this.waiting = resolve; });
    return JSON.parse(line);
  }
}

let link = null;
let profiles = [];
let running = false;

function setLink(text, kind) {
  const b = $("link");
  b.textContent = text;
  b.className = "jp-badge" + (kind ? " jp-badge--" + kind : "");
}

function enable() {
  const ready = link !== null && !running && profiles.length > 0;
  $("profile").disabled = !ready;
  $("quick").disabled = !ready;
  $("full").disabled = !ready;
  $("busy").hidden = !running;
}

function size(r) {
  const kbits = r.bits / 1024;
  return kbits >= 8192 ? Math.floor(kbits / 8192) + " MB" : kbits + " Kbit";
}

function stat(label, value, meta) {
  const tile = document.createElement("div");
  tile.className = "jp-stat";
  const l = document.createElement("span");
  l.className = "jp-stat__label";
  l.textContent = label;
  const v = document.createElement("span");
  v.className = "jp-stat__value";
  v.textContent = value;
  tile.append(l, v);
  if (meta) {
    const m = document.createElement("span");
    m.className = "jp-stat__meta";
    m.textContent = meta;
    tile.append(m);
  }
  return tile;
}

function row(body, cells, numeric) {
  const tr = document.createElement("tr");
  cells.forEach((c, i) => {
    const td = document.createElement("td");
    if (c instanceof Node) td.append(c); else td.textContent = c;
    if (numeric && numeric.includes(i)) td.className = "jp-num";
    tr.append(td);
  });
  body.append(tr);
  return tr;
}

const VERDICT = {
  pass: { word: "PASS", badge: "Pass", kind: "ok" },
  fail: { word: "FAIL", badge: "Fail", kind: "danger" },
  no_device: { word: "NO DEVICE", badge: "Nothing answers", kind: "warn" },
};

function badge(v) {
  const b = document.createElement("span");
  b.className = "jp-badge jp-badge--" + VERDICT[v].kind;
  b.textContent = VERDICT[v].badge;
  return b;
}

function wireText(w) {
  if (w.kind === "short") return w.line + " is shorted to " + w.other;
  return w.line + (w.kind === "stuck_low" ? " is stuck low" : " is stuck high");
}

function badLines(hex) {
  const bits = BigInt(hex);
  const out = [];
  for (let i = 0n; i < 40n; i++) if (bits >> i & 1n) out.push("DQ" + i);
  return out.join(" ");
}

function show(answer) {
  const r = answer.report;
  const v = VERDICT[r.verdict];
  $("empty").hidden = true;
  $("result").hidden = false;
  $("verdict").textContent = v.word;
  $("verdict-badge").textContent = v.badge;
  $("verdict-badge").className = "jp-badge jp-badge--" + v.kind;
  $("result-what").textContent = r.profile + ", " + (answer.test === "full" ? "full test" : "quick test");

  let title = "", text = "";
  if (r.power_fault) {
    title = "Supply fault";
    text = "The module drew " + r.current_ma + " mA, or its supply did not come up. Take it out and check which way round it is.";
  } else if (r.wire_faults.length) {
    title = "Wiring fault on the cartridge";
    text = r.wire_faults.map(wireText).join(". ") + ". Look for a bent socket pin or a solder bridge on that line.";
  } else if (r.failure) {
    title = "Memory fault";
    text = r.failure.errors + " errors in the " + r.failure.phase + " phase. The module is faulty.";
  } else if (r.no_device) {
    title = "";
  }
  $("problem").hidden = !title;
  $("problem-title").textContent = title;
  $("problem-text").textContent = text;

  const stats = $("stats");
  stats.replaceChildren();
  if (r.words) {
    stats.append(stat("Size", size(r), r.words / 1024 + "K x" + r.width));
    stats.append(stat("Address", r.row_bits + " x " + r.col_bits, "row x column bits"));
    if (r.mode) stats.append(stat("Mode", r.mode));
    if (r.t_rac_ns !== null) stats.append(stat("Access time", r.t_rac_ns + " ns", "from RAS"));
    stats.append(stat("Supply current", r.current_ma + " mA"));
    if (r.retention_ms !== null) stats.append(stat("Retention", r.retention_ms + " ms", "or longer, without refresh"));
  }

  const rows = $("detail-rows");
  rows.replaceChildren();
  if (r.failure) {
    row(rows, ["Phase", r.failure.phase]);
    row(rows, ["Errors", String(r.failure.errors)]);
    row(rows, ["First address", "0x" + r.failure.addr.toString(16)]);
    row(rows, ["Bad bits", r.failure.bits]);
    row(rows, ["Data read", r.failure.read]);
    row(rows, ["Failing lines", badLines(r.failure.bits)]);
    rows.querySelectorAll("td:last-child").forEach((td) => td.classList.add("jp-mono"));
  }
  $("detail").hidden = !r.failure;

  $("history-empty").hidden = true;
  $("history-wrap").hidden = false;
  const tr = row($("history"), [
    new Date().toLocaleTimeString(), r.profile, answer.test === "full" ? "Full" : "Quick", badge(r.verdict),
    r.words ? size(r) : "", r.t_rac_ns !== null ? r.t_rac_ns + " ns" : "", r.words ? r.current_ma + " mA" : "",
  ], [5, 6]);
  $("history").prepend(tr);
}

async function run(test) {
  running = true;
  enable();
  try {
    const answer = await link.send(test + " " + $("profile").value);
    if (!answer.ok) throw new Error(answer.error);
    show(answer);
    setLink("Connected over " + link.name, "ok");
  } catch (e) {
    setLink("Lost the tester: " + e.message, "danger");
  }
  running = false;
  enable();
}

async function start(l) {
  link = l;
  try {
    const list = await link.send("list");
    profiles = list.profiles;
    const sel = $("profile");
    sel.replaceChildren();
    for (const p of profiles) {
      const o = document.createElement("option");
      o.value = p.n;
      o.textContent = p.name;
      sel.append(o);
    }
    hint();
    const id = await link.send("id");
    $("engine").textContent = id.engine + (id.engine_ok ? "" : " (unexpected)");
    $("tick").textContent = id.tick_ps + " ps";
    $("unit").hidden = false;
    setLink("Connected over " + link.name, id.engine_ok ? "ok" : "warn");
  } catch (e) {
    link = null;
    setLink("The tester does not answer: " + e.message, "danger");
  }
  enable();
}

function hint() {
  const p = profiles[Number($("profile").value)];
  $("profile-hint").textContent = p && p.aux_rails
    ? "This chip also gets -5 V and +12 V. Check it is in the 4116 socket." : "";
}

async function wifiPanel(http) {
  const s = await http.state();
  if (!s.wifi) return;
  $("wifi").hidden = false;
  $("wifi-now").textContent = s.wifi.mode === "ap"
    ? "The module has its own network, " + s.wifi.ssid + ". Join it to a network here."
    : "Joined to " + s.wifi.ssid + " at " + s.wifi.ip + ".";
  $("ssid").value = s.wifi.mode === "ap" ? "" : s.wifi.ssid;
  $("wifi-form").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    await fetch("api/wifi", { method: "POST", body: new URLSearchParams(new FormData($("wifi-form"))) });
    $("wifi-saved").hidden = false;
  });
}

async function boot() {
  $("quick").addEventListener("click", () => run("basic"));
  $("full").addEventListener("click", () => run("full"));
  $("profile").addEventListener("change", hint);
  const http = new HttpLink();
  try {
    await http.state();
    await start(http);
    await wifiPanel(http);
    return;
  } catch (e) { /* not served by a module: offer USB */ }
  if (!("serial" in navigator)) {
    setLink("No tester", "warn");
    $("no-serial").hidden = false;
    return;
  }
  setLink("No tester connected", "");
  $("usb").hidden = false;
  $("usb-connect").addEventListener("click", async () => {
    try {
      const port = await navigator.serial.requestPort();
      const serial = new SerialLink(port);
      await serial.open();
      $("usb").hidden = true;
      await start(serial);
    } catch (e) {
      setLink("Could not open the port: " + e.message, "danger");
    }
  });
}

boot();
