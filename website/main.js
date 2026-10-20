/* ==========================================================================
   NOCTURNE MK-IV — PUBLIC SITE TELEMETRY & LULL ORCHESTRATOR
   Watches the gaps between your inputs.
   ========================================================================== */
(() => {
  "use strict";

  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));
  const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
  const pad2 = (n) => String(n).padStart(2, "0");

  // ---------- Elements ----------
  const navMg = $("#nav-mg");
  const navStatus = $(".nav__status");
  const heroTime = $("#hero-time");
  const heroIdle = $("#hero-idle");
  const audioToggle = $("#audio-toggle");
  const audioLabel = $(".audio-label", audioToggle);
  const audioNote = $("#audio-note");
  const segMg = $("#seg-mg");
  const segWrap = $(".seg");
  const kii = $("#kii");
  const kiiLeds = $$(".kii__led", kii);
  const crtCanvas = $("#crt");
  const crtWrap = $(".crt");
  const crtState = $("#crt-state");
  const lampLull = $("#lamp-lull");
  const lampEvent = $("#lamp-event");
  const tctlEl = $("#tctl");
  const coldspotEl = $("#coldspot");
  const timelineSteps = $$(".timeline__step");
  const specimenCards = $$(".specimen");
  const modeBtns = $$("[data-mode]");
  const vigilSection = $("#vigil");
  const vigilArc = $("#vigil-arc");
  const dialTicks = $("#dial-ticks");
  const dialLabels = $("#dial-labels");
  const dialNeedle = $("#dial-needle");
  const vigilBadge = $("#vigil-badge");
  const vigilMsg = $("#vigil-msg");
  const escSlots = $$(".esc-slot", $("#esc-slots"));
  const escWindow = $("#esc-window");
  const exorciseBtn = $("#exorcise-btn");
  const exorciseMsg = $("#exorcise-msg");
  const exorciseFlash = $("#exorcise-flash");
  const logEl = $("#log");
  const dwellEl = $("#dwell");
  const eyesEl = $("#eyes");
  const whispersEl = $("#whispers");
  const phantomEl = $("#phantom");
  const grainCanvas = $("#grain");
  const planchettePath = $(".planchette__path");
  const planchetteDot = $(".planchette__dot");

  // ---------- State ----------
  const startTime = performance.now();
  let lastInputAt = performance.now();
  let lastKeyAt = 0;
  let idleSec = 0;
  let exorcisedUntil = 0;
  let currentMg = 1.5;
  let targetMg = 1.5;
  let eventEnergy = 0;
  let activeSpecimen = null;
  let currentSpectralMode = null;
  let pointerX = window.innerWidth * 0.5;
  let pointerY = window.innerHeight * 0.4;
  let phantomX = pointerX;
  let phantomY = pointerY;
  let lullTriggered25 = false;
  let lullTriggered40 = false;
  let lullTriggered72 = false;
  let lullTriggered150 = false;

  const escRing = [];
  const scopeBuf = new Float32Array(64);
  const baseTitle = document.title;

  const WHISPER_LINES = [
    "something is behind your chair",
    "you stopped typing",
    "18.4 Hz · standing wave",
    "did the hallway light just change?",
    "0x1B · 0x1B · 0x1B",
    "it only listens to the silence",
    "look at the corner of the monitor",
    "22.40 mG — REDLINE SPIKE",
    "don't turn around yet",
    "who pressed that key?"
  ];

  // ---------- Incident Audit Log ----------
  function appendLog(msg, cls = "") {
    if (!logEl) return;
    const now = new Date();
    const ts = `${pad2(now.getHours())}:${pad2(now.getMinutes())}:${pad2(now.getSeconds())}.${String(now.getMilliseconds()).padStart(3, "0")}`;
    const li = document.createElement("li");
    const tSpan = document.createElement("span");
    tSpan.className = "t";
    tSpan.textContent = `[${ts}] `;
    const mSpan = document.createElement("span");
    if (cls) mSpan.className = cls;
    mSpan.textContent = msg;
    li.append(tSpan, mSpan);
    logEl.appendChild(li);
    while (logEl.children.length > 48) {
      logEl.removeChild(logEl.firstChild);
    }
    logEl.scrollTop = logEl.scrollHeight;
  }

  // Seed initial log entries
  appendLog("nocturne-mk-iv daemon v1.0.4-RELEASE initialized (AirGapEnforcer: ARMED)");
  appendLog("POSIX AF_INET / AF_INET6 descriptor scrub: 0 open sockets");
  appendLog("SQLite WAL telemetry store mounted (rolling cap: 10,000 rows)");
  appendLog("observer session detected on viewport — idle hooks attached", "you");

  // ---------- Input & Lull Tracking ----------
  function noteInput(kind) {
    const wasIdle = idleSec >= 4.0;
    lastInputAt = performance.now();
    if (wasIdle) {
      document.body.classList.add("resumed");
      setTimeout(() => document.body.classList.remove("resumed"), 220);
      if (idleSec >= 7.2) {
        appendLog(`input resumed after ${idleSec.toFixed(1)}s lull (${kind}) — receding to ambient`, "you");
      }
    }
    idleSec = 0;
    lullTriggered25 = false;
    lullTriggered40 = false;
    lullTriggered72 = false;
    lullTriggered150 = false;
    document.body.classList.remove("is-idle");
    document.documentElement.style.setProperty("--idle", "0");
    if (document.title !== baseTitle) document.title = baseTitle;
  }

  window.addEventListener("pointermove", (e) => {
    pointerX = e.clientX;
    pointerY = e.clientY;
    noteInput("pointer");
    updateEyes(e.clientX, e.clientY);
  }, { passive: true });

  window.addEventListener("pointerdown", () => noteInput("click"), { passive: true });
  window.addEventListener("wheel", () => noteInput("scroll"), { passive: true });
  window.addEventListener("touchstart", () => noteInput("touch"), { passive: true });

  // ---------- Triple-ESC Hardware Exorcise Circuit ----------
  function triggerExorcise(source) {
    exorcisedUntil = performance.now() + 8000;
    if (window.NocturneAudio) {
      window.NocturneAudio.sever();
    }
    syncAudioUI(false);
    document.documentElement.classList.remove("mode-0", "mode-1");
    currentSpectralMode = null;
    modeBtns.forEach((b) => {
      b.setAttribute("aria-pressed", "false");
      b.textContent = "Let it into this page";
    });
    specimenCards.forEach((c) => {
      c.classList.remove("playing");
      const btn = $(".play", c);
      if (btn) btn.textContent = "▶ Play specimen";
    });
    activeSpecimen = null;
    eventEnergy = 0;
    targetMg = 1.5;
    currentMg = 1.5;
    crtWrap.classList.remove("tear");
    document.body.classList.remove("shake", "is-idle");
    document.documentElement.style.setProperty("--idle", "0");
    phantomEl.classList.remove("on");
    whispersEl.innerHTML = "";

    if (exorciseFlash) {
      exorciseFlash.classList.add("on");
      setTimeout(() => exorciseFlash.classList.remove("on"), 650);
    }
    if (exorciseMsg) {
      exorciseMsg.textContent = `EXORCISE PURGE EXECUTED (${source}) — NonHookStasis active, LUT & ringbuffer restored.`;
    }
    appendLog(`EXORCISE PURGE CIRCUIT TRIPPED via ${source} (< 50ms) — NonHookStasis engaged`, "crit");
  }

  window.addEventListener("keydown", (e) => {
    lastKeyAt = performance.now();
    noteInput("key");

    // Strictly filter for 0x1B (Escape); discard all other keys unread
    if (e.key === "Escape") {
      const now = performance.now();
      while (escRing.length && now - escRing[0] > 800) {
        escRing.shift();
      }
      escRing.push(now);

      escSlots.forEach((slot, idx) => {
        slot.classList.toggle("hit", idx < escRing.length);
      });
      if (escWindow) {
        escWindow.classList.remove("run");
        void escWindow.offsetWidth;
        escWindow.classList.add("run");
      }

      appendLog(`0x1B [ESC] captured in failsafe ringbuffer (${escRing.length}/3 within 800ms)`, "warn");

      if (escRing.length >= 3) {
        escRing.length = 0;
        setTimeout(() => escSlots.forEach((s) => s.classList.remove("hit")), 420);
        triggerExorcise("3× ESC < 800ms");
      } else {
        setTimeout(() => {
          const t = performance.now();
          while (escRing.length && t - escRing[0] > 800) escRing.shift();
          escSlots.forEach((slot, idx) => slot.classList.toggle("hit", idx < escRing.length));
        }, 820);
      }
    }
  });

  if (exorciseBtn) {
    exorciseBtn.addEventListener("click", () => triggerExorcise("MANUAL PURGE CIRCUIT"));
  }

  // ---------- Audio Controls & Specimen Cards ----------
  function syncAudioUI(on) {
    if (audioToggle) {
      audioToggle.setAttribute("aria-pressed", on ? "true" : "false");
    }
    if (audioLabel) {
      audioLabel.textContent = on ? "Listening (mute)" : "Let it speak";
    }
    if (audioNote) {
      audioNote.textContent = on
        ? "48.0 kHz stereo ringbuffer armed. Specimens will also manifest during lulls."
        : "Audio is asleep. Press any specimen to wake it.";
    }
  }

  if (audioToggle) {
    audioToggle.addEventListener("click", async () => {
      if (!window.NocturneAudio) return;
      if (window.NocturneAudio.on) {
        await window.NocturneAudio.sleep();
        syncAudioUI(false);
        appendLog("cpal stereo ringbuffer suspended by operator");
      } else {
        const ok = await window.NocturneAudio.wake();
        syncAudioUI(ok);
        if (ok) {
          appendLog("cpal 48.0 kHz stereo ringbuffer armed (-44.2 dBFS floor)", "warn");
          dispatchSpecimen("whisper", "manual wake");
        }
      }
    });
  }

  const SPECIMEN_META = {
    creak:   { label: "Spatial Floorboard Creak", mg: 11.4, tear: false },
    key:     { label: "Phantom Keystroke Echo", mg: 4.8, tear: false },
    whisper: { label: "3-Formant Spirit-Box EVP Whisper", mg: 14.2, tear: false },
    knock:   { label: "Wall-Stud Double-Knock + Reed Relay", mg: 18.6, tear: false },
    hum:     { label: "Redline 55/110/165 Hz Mains Hum", mg: 22.4, tear: true },
    breath:  { label: "18–24 Hz Infrasonic Room Breath", mg: 9.8, tear: false },
  };

  async function dispatchSpecimen(name, reason = "manual") {
    if (!window.NocturneAudio) return;
    if (!window.NocturneAudio.on) {
      const ok = await window.NocturneAudio.wake();
      syncAudioUI(ok);
    }
    const meta = SPECIMEN_META[name] || { label: name, mg: 12.0, tear: false };
    const thermal = clamp((idleSec / 20) + 0.35, 0.2, 0.95);
    const dur = window.NocturneAudio.play(name, { thermal }) || 1.8;

    activeSpecimen = name;
    eventEnergy = Math.max(eventEnergy, meta.mg >= 20 ? 1.0 : 0.72);
    targetMg = Math.max(targetMg, meta.mg);

    specimenCards.forEach((c) => {
      const isThis = c.dataset.specimen === name;
      c.classList.toggle("playing", isThis);
      const btn = $(".play", c);
      if (btn) btn.textContent = isThis ? "■ Manifesting…" : "▶ Play specimen";
    });

    if (meta.tear) {
      crtWrap.classList.add("tear");
      document.body.classList.add("shake");
    }

    appendLog(
      `dispatch_${name} [${reason}] — peak ${meta.mg.toFixed(2)} mG · dur ${dur.toFixed(2)}s`,
      meta.mg >= 20 ? "crit" : "warn"
    );

    setTimeout(() => {
      if (activeSpecimen === name) {
        activeSpecimen = null;
        crtWrap.classList.remove("tear");
        document.body.classList.remove("shake");
        specimenCards.forEach((c) => {
          c.classList.remove("playing");
          const btn = $(".play", c);
          if (btn) btn.textContent = "▶ Play specimen";
        });
      }
    }, dur * 1000);
  }

  specimenCards.forEach((card) => {
    card.addEventListener("pointermove", (e) => {
      const r = card.getBoundingClientRect();
      card.style.setProperty("--mx", `${((e.clientX - r.left) / r.width) * 100}%`);
      card.style.setProperty("--my", `${((e.clientY - r.top) / r.height) * 100}%`);
    });
    const btn = $(".play", card);
    if (btn) {
      btn.addEventListener("click", () => {
        dispatchSpecimen(card.dataset.specimen, "specimen card");
      });
    }
  });

  // ---------- Spectral Color-Shift Modes ----------
  function setSpectralMode(mode) {
    const html = document.documentElement;
    if (currentSpectralMode === mode) {
      html.classList.remove("mode-0", "mode-1");
      currentSpectralMode = null;
      appendLog("hardware gamma LUT restored to unperturbed baseline");
    } else {
      html.classList.remove("mode-0", "mode-1");
      html.classList.add(`mode-${mode}`);
      currentSpectralMode = mode;
      const desc = mode === "0"
        ? "Mode 0 — Ectoplasmic Cyan-Violet Phase Inversion (hue-rotate 155deg)"
        : "Mode 1 — Crimson Nightmare Solarization (hue-rotate -55deg)";
      appendLog(`spectral LUT perturbation engaged: ${desc}`, "warn");
    }
    modeBtns.forEach((b) => {
      const active = b.dataset.mode === currentSpectralMode;
      b.setAttribute("aria-pressed", active ? "true" : "false");
      b.textContent = active ? "Restore baseline LUT" : "Let it into this page";
    });
  }

  modeBtns.forEach((btn) => {
    btn.addEventListener("click", () => setSpectralMode(btn.dataset.mode));
  });

  // ---------- 24-Hour Witching Hour Vigil Chronometer ----------
  function polarToCartesian(cx, cy, r, angleDeg) {
    const rad = ((angleDeg - 90) * Math.PI) / 180;
    return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
  }

  function buildVigilDial() {
    if (!dialTicks || !dialLabels || !vigilArc) return;
    // 00:00 (0 deg) to 05:00 (5/24 * 360 = 75 deg)
    const start = polarToCartesian(150, 150, 128, 0);
    const end = polarToCartesian(150, 150, 128, (5 / 24) * 360);
    vigilArc.setAttribute("d", `M ${start.x.toFixed(2)} ${start.y.toFixed(2)} A 128 128 0 0 1 ${end.x.toFixed(2)} ${end.y.toFixed(2)}`);

    for (let h = 0; h < 24; h++) {
      const deg = (h / 24) * 360;
      const major = h % 3 === 0;
      const p1 = polarToCartesian(150, 150, major ? 114 : 120, deg);
      const p2 = polarToCartesian(150, 150, 128, deg);
      const line = document.createElementNS("http://www.w3.org/2000/svg", "line");
      line.setAttribute("x1", p1.x.toFixed(2));
      line.setAttribute("y1", p1.y.toFixed(2));
      line.setAttribute("x2", p2.x.toFixed(2));
      line.setAttribute("y2", p2.y.toFixed(2));
      line.setAttribute("class", major ? "dial__tick major" : "dial__tick");
      dialTicks.appendChild(line);

      if (major) {
        const lp = polarToCartesian(150, 150, 100, deg);
        const txt = document.createElementNS("http://www.w3.org/2000/svg", "text");
        txt.setAttribute("x", lp.x.toFixed(2));
        txt.setAttribute("y", lp.y.toFixed(2));
        txt.setAttribute("class", "dial__label");
        txt.textContent = `${pad2(h)}:00`;
        dialLabels.appendChild(txt);
      }
    }
  }

  function updateVigilClock() {
    const now = new Date();
    const h = now.getHours();
    const m = now.getMinutes();
    const s = now.getSeconds();
    const timeStr = `${pad2(h)}:${pad2(m)}:${pad2(s)}`;
    if (heroTime) heroTime.textContent = timeStr;

    const totalHours = h + m / 60 + s / 3600;
    const deg = (totalHours / 24) * 360;
    if (dialNeedle) {
      dialNeedle.style.transform = `rotate(${deg.toFixed(2)}deg)`;
    }

    const inWitching = h >= 0 && h < 5;
    if (vigilSection) vigilSection.classList.toggle("active", inWitching);
    if (vigilBadge && vigilMsg) {
      if (inWitching) {
        vigilBadge.textContent = "VIGIL ARMED · WITCHING WINDOW OPEN";
        vigilMsg.textContent = `It is ${pad2(h)}:${pad2(m)}. You are inside the 00:00–05:00 bracket. The daemon is awake.`;
      } else {
        const hoursUntilMidnight = 23 - h;
        const minsUntilMidnight = 59 - m;
        vigilBadge.textContent = "DORMANT · AWAITING MIDNIGHT";
        vigilMsg.textContent = `It is ${pad2(h)}:${pad2(m)}. The vigil opens in ${hoursUntilMidnight}h ${minsUntilMidnight}m (or whenever you toggle Curfew Override).`;
      }
    }
  }

  buildVigilDial();
  updateVigilClock();

  // ---------- Whisper Spawner & Text Glitch ----------
  function spawnWhisper(text, red = false) {
    if (!whispersEl || performance.now() < exorcisedUntil) return;
    const span = document.createElement("span");
    span.className = red ? "whisper red" : "whisper";
    span.textContent = text || WHISPER_LINES[(Math.random() * WHISPER_LINES.length) | 0];
    span.style.left = `${8 + Math.random() * 72}%`;
    span.style.top = `${12 + Math.random() * 76}%`;
    whispersEl.appendChild(span);
    setTimeout(() => span.remove(), 6000);
  }

  const hauntWords = $$(".haunt");
  setInterval(() => {
    if (performance.now() < exorcisedUntil) return;
    if (Math.random() < (idleSec > 4 ? 0.65 : 0.25)) {
      const el = hauntWords[(Math.random() * hauntWords.length) | 0];
      if (!el) return;
      const orig = el.dataset.orig || el.textContent;
      el.dataset.orig = orig;
      const alt = el.dataset.alt || orig;
      el.textContent = alt;
      el.classList.add("glitch");
      setTimeout(() => {
        el.textContent = orig;
        el.classList.remove("glitch");
      }, 190 + Math.random() * 240);
    }
  }, 3200);

  // ---------- Footer Eyes Tracking ----------
  function updateEyes(x, y) {
    if (!eyesEl) return;
    const r = eyesEl.getBoundingClientRect();
    const dx = clamp((x - (r.left + r.width / 2)) / 300, -1, 1);
    const px = 50 + dx * 28;
    eyesEl.style.setProperty("--px", `${px.toFixed(1)}%`);
  }

  if (eyesEl && "IntersectionObserver" in window) {
    const io = new IntersectionObserver((entries) => {
      entries.forEach((e) => {
        if (e.isIntersecting) eyesEl.classList.add("open");
      });
    }, { threshold: 0.3 });
    io.observe(eyesEl);
  } else if (eyesEl) {
    eyesEl.classList.add("open");
  }

  // ---------- 150ms Telemetry & Lull Sweep (matches daemon cadence) ----------
  const crtCtx = crtCanvas ? crtCanvas.getContext("2d") : null;
  let sweepTick = 0;

  setInterval(() => {
    sweepTick++;
    const now = performance.now();
    const isExorcised = now < exorcisedUntil;

    // Dwell counter
    const dwellSec = Math.floor((now - startTime) / 1000);
    if (dwellEl) {
      dwellEl.textContent = `${pad2(Math.floor(dwellSec / 60))}:${pad2(dwellSec % 60)}`;
    }

    if (sweepTick % 6 === 0) updateVigilClock();

    // Idle calculation
    idleSec = isExorcised ? 0 : (now - lastInputAt) / 1000;
    if (heroIdle) heroIdle.textContent = idleSec.toFixed(1);

    const idleNorm = clamp((idleSec - 2.0) / 15.0, 0, 1);
    document.documentElement.style.setProperty("--idle", idleNorm.toFixed(3));
    document.body.classList.toggle("is-idle", idleSec >= 4.0);

    // Highlight lull timeline steps
    timelineSteps.forEach((step) => {
      const threshold = parseFloat(step.dataset.t || "0");
      step.classList.toggle("lit", idleSec >= threshold);
    });

    // Progressive Lull Manifestations
    if (!isExorcised) {
      if (idleSec >= 2.5 && !lullTriggered25) {
        lullTriggered25 = true;
        targetMg = Math.max(targetMg, 3.4);
        if (now - lastKeyAt < 6000 && window.NocturneAudio && window.NocturneAudio.on) {
          dispatchSpecimen("key", "2.5s cadence halt");
        } else {
          appendLog("2.5s cadence halt threshold crossed — phantom switch armed", "warn");
        }
      }
      if (idleSec >= 4.0 && !lullTriggered40) {
        lullTriggered40 = true;
        targetMg = Math.max(targetMg, 8.2);
        spawnWhisper("you stopped moving");
        appendLog("4.0s cognitive lull window OPEN — REM-Pod array armed", "warn");
      }
      if (idleSec >= 7.2 && !lullTriggered72) {
        lullTriggered72 = true;
        targetMg = Math.max(targetMg, 14.8);
        spawnWhisper("something shifted behind you", true);
        if (window.NocturneAudio && window.NocturneAudio.on) {
          const pool = ["creak", "whisper", "knock", "breath"];
          dispatchSpecimen(pool[(Math.random() * pool.length) | 0], "7.2s deep lull");
        } else {
          appendLog("7.2s deep lull — spatial stick-slip resonance queued", "warn");
        }
      }
      if (idleSec >= 15.0 && !lullTriggered150) {
        lullTriggered150 = true;
        targetMg = 22.4;
        eventEnergy = 1.0;
        document.title = "Nocturne — still there?";
        spawnWhisper("22.40 mG · IT KNOWS YOUR RHYTHM", true);
        appendLog("15.0s witching lull — planchette Lissajous drift + LUT bend engaged", "crit");
        if (window.NocturneAudio && window.NocturneAudio.on) {
          dispatchSpecimen("hum", "15.0s redline spike");
        }
      }
    }

    // EMF decay & jitter
    eventEnergy = Math.max(0, eventEnergy - 0.035);
    const ambientTarget = 1.5 + idleNorm * 12.5 + (Math.random() - 0.48) * 0.45;
    targetMg = Math.max(ambientTarget, targetMg * 0.94);
    currentMg = clamp(currentMg + (targetMg - currentMg) * 0.28, 1.2, 22.4);
    if (eventEnergy > 0.85) currentMg = 22.4;

    const mgStr = currentMg.toFixed(2).padStart(5, "0");
    if (segMg) segMg.textContent = mgStr;
    if (navMg) navMg.textContent = currentMg.toFixed(2);
    const isHot = currentMg >= 15.0;
    if (segWrap) segWrap.classList.toggle("hot", isHot);
    if (navStatus) navStatus.classList.toggle("hot", isHot);

    // K-II 5-step LED scale (1.5, 2.5, 10, 20, 20+ SPIKE)
    const level =
      currentMg >= 20.5 ? 5 :
      currentMg >= 15.0 ? 4 :
      currentMg >= 8.0  ? 3 :
      currentMg >= 2.5  ? 2 : 1;
    if (kii) kii.setAttribute("aria-valuenow", String(level));
    kiiLeds.forEach((led, i) => led.classList.toggle("on", i < level));

    // Lamps & thermocouple readout
    if (lampLull) lampLull.classList.toggle("on", idleSec >= 4.0);
    if (lampEvent) lampEvent.classList.toggle("on", currentMg >= 10.0 || activeSpecimen !== null);

    const tctl = 41.2 + Math.sin(now * 0.0007) * 1.4 + eventEnergy * 4.2;
    const coldDip = -(idleNorm * 4.1 + eventEnergy * 1.8);
    if (tctlEl) tctlEl.textContent = tctl.toFixed(1);
    if (coldspotEl) coldspotEl.textContent = coldDip.toFixed(1);

    if (crtState) {
      crtState.textContent =
        isExorcised ? "STASIS (EXORCISED)" :
        currentMg >= 20.0 ? "REDLINE SPIKE 22.40mG" :
        activeSpecimen ? `MANIFEST: ${activeSpecimen.toUpperCase()}` :
        idleSec >= 4.0 ? "LULL ARMED" : "AMBIENT";
    }

    // Draw 64-point CRT EVP Spectrograph
    drawCRT(now, isExorcised);
  }, 150);

  function drawCRT(now, isExorcised) {
    if (!crtCtx || !crtCanvas) return;
    const w = crtCanvas.width;
    const h = crtCanvas.height;

    // Phosphor persistence fade
    crtCtx.fillStyle = "rgba(3, 16, 8, 0.34)";
    crtCtx.fillRect(0, 0, w, h);

    // Subtle graticule grid
    crtCtx.strokeStyle = "rgba(35, 232, 138, 0.08)";
    crtCtx.lineWidth = 1;
    crtCtx.beginPath();
    for (let x = 0; x <= w; x += w / 8) {
      crtCtx.moveTo(x, 0);
      crtCtx.lineTo(x, h);
    }
    for (let y = 0; y <= h; y += h / 4) {
      crtCtx.moveTo(0, y);
      crtCtx.lineTo(w, y);
    }
    crtCtx.stroke();

    // Acquire 64-point time-domain samples from Web Audio or procedural fallback
    const hasAudio = window.NocturneAudio && window.NocturneAudio.scope(scopeBuf);
    const amp = isExorcised ? 0.01 : clamp((currentMg - 1.0) / 18.0, 0.06, 1.0);

    if (!hasAudio) {
      for (let i = 0; i < 64; i++) {
        const phase = now * 0.008 + i * 0.38;
        const carrier = Math.sin(phase) * 0.35 + Math.sin(phase * 2.7) * 0.25;
        const burst = (Math.random() - 0.5) * (0.35 + amp * 1.35);
        scopeBuf[i] = (carrier + burst) * amp;
      }
    }

    const tearOffset = currentMg >= 20.0 ? (Math.random() - 0.5) * 18 : 0;

    // Outer phosphor bloom + core trace
    crtCtx.save();
    crtCtx.strokeStyle = currentMg >= 20.0 ? "#ff4a3d" : "#23e88a";
    crtCtx.shadowColor = currentMg >= 20.0 ? "rgba(255, 74, 61, 0.85)" : "rgba(35, 232, 138, 0.85)";
    crtCtx.shadowBlur = 14;
    crtCtx.lineWidth = 2.2;
    crtCtx.beginPath();

    for (let i = 0; i < 64; i++) {
      const x = (i / 63) * w + (i > 24 && i < 42 ? tearOffset : 0);
      const v = clamp(scopeBuf[i], -1, 1);
      const y = h * 0.5 + v * (h * 0.42);
      if (i === 0) crtCtx.moveTo(x, y);
      else crtCtx.lineTo(x, y);
    }
    crtCtx.stroke();

    // Vertical spectral bars for authentic MK-IV CRT look
    crtCtx.strokeStyle = currentMg >= 20.0 ? "rgba(255, 74, 61, 0.32)" : "rgba(35, 232, 138, 0.28)";
    crtCtx.lineWidth = 2;
    crtCtx.shadowBlur = 0;
    crtCtx.beginPath();
    for (let i = 0; i < 64; i++) {
      const x = (i / 63) * w;
      const spread = Math.abs(scopeBuf[i]) * (h * 0.44);
      crtCtx.moveTo(x, h * 0.5 - spread);
      crtCtx.lineTo(x, h * 0.5 + spread);
    }
    crtCtx.stroke();
    crtCtx.restore();
  }

  // ---------- 60fps Visual Loop (Film Grain, Planchette Dot, Phantom Cursor) ----------
  const gCtx = grainCanvas ? grainCanvas.getContext("2d") : null;
  if (grainCanvas) {
    grainCanvas.width = 160;
    grainCanvas.height = 160;
  }
  const pathLen = planchettePath ? planchettePath.getTotalLength() : 0;
  let frameCount = 0;

  function animate(t) {
    frameCount++;

    // Film grain refresh every 4 frames
    if (gCtx && frameCount % 4 === 0) {
      const img = gCtx.createImageData(160, 160);
      const d = img.data;
      for (let i = 0; i < d.length; i += 4) {
        const v = (Math.random() * 255) | 0;
        d[i] = v;
        d[i + 1] = v;
        d[i + 2] = v;
        d[i + 3] = 38;
      }
      gCtx.putImageData(img, 0, 0);
    }

    // Planchette SVG figure-8 dot
    if (planchettePath && planchetteDot && pathLen > 0) {
      const pt = planchettePath.getPointAtLength(((t * 0.045) % pathLen));
      planchetteDot.setAttribute("cx", pt.x.toFixed(1));
      planchetteDot.setAttribute("cy", pt.y.toFixed(1));
    }

    // Phantom cursor Lissajous figure-8 micro-drift
    if (phantomEl && t > exorcisedUntil) {
      const showPhantom = idleSec >= 3.2;
      phantomEl.classList.toggle("on", showPhantom);
      const lissX = Math.sin(t * 0.0017) * (18 + idleSec * 2.4);
      const lissY = Math.sin(t * 0.0034) * (11 + idleSec * 1.6);
      phantomX += (pointerX + lissX - phantomX) * 0.06;
      phantomY += (pointerY + lissY - phantomY) * 0.06;
      phantomEl.style.transform = `translate3d(${phantomX.toFixed(1)}px, ${phantomY.toFixed(1)}px, 0)`;
    }

    requestAnimationFrame(animate);
  }
  requestAnimationFrame(animate);
})();
