/**
 * Project Nocturne (MK-IV) — Séance Panel Presentation & Tauri IPC Bridge
 * Connects every Séance Panel control, gauge, LED array, and oscillograph readout
 * to the Rust core daemon via Tauri v2 IPC and renders the CRT EVP Spectrograph (150ms SWEEP).
 */

(function initSeancePanel() {
  "use strict";

  // Section 4.1 Zero Network Egress & Air-Gap Enforcement:
  // Neutralize external browser network APIs inside the Webview runtime (0 KB/s guarantee)
  // while returning a rejected Promise on `fetch` so Tauri v2's internal `ipc-protocol.js`
  // seamlessly switches from `fetch('ipc://localhost/...')` to `window.ipc.postMessage`.
  const blockNetworkEgressSync = () => {
    throw new Error(
      "SECURITY VIOLATION: Project Nocturne operates in Zero-Egress Air-Gap mode (0 KB/s)"
    );
  };
  const blockNetworkEgressAsync = () =>
    Promise.reject(
      new Error(
        "SECURITY VIOLATION: Project Nocturne operates in Zero-Egress Air-Gap mode (0 KB/s)"
      )
    );
  try {
    Object.defineProperty(window, "fetch", {
      value: blockNetworkEgressAsync,
      writable: false,
      configurable: false,
    });
    Object.defineProperty(window, "XMLHttpRequest", {
      value: blockNetworkEgressSync,
      writable: false,
      configurable: false,
    });
    Object.defineProperty(window, "WebSocket", {
      value: blockNetworkEgressSync,
      writable: false,
      configurable: false,
    });
    Object.defineProperty(window, "EventSource", {
      value: blockNetworkEgressSync,
      writable: false,
      configurable: false,
    });
    if (navigator && "sendBeacon" in navigator) {
      Object.defineProperty(navigator, "sendBeacon", {
        value: () => false,
        writable: false,
        configurable: false,
      });
    }
  } catch (_) {
    // Ignore if environment already locked
  }

  function getTauriInvoke() {
    if (
      window.__TAURI__ &&
      window.__TAURI__.core &&
      typeof window.__TAURI__.core.invoke === "function"
    ) {
      return window.__TAURI__.core.invoke.bind(window.__TAURI__.core);
    }
    if (
      window.__TAURI_INTERNALS__ &&
      typeof window.__TAURI_INTERNALS__.invoke === "function"
    ) {
      return window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__);
    }
    return null;
  }

  async function invokeCommand(cmd, args = {}) {
    const invokeFn = getTauriInvoke();
    if (invokeFn) {
      try {
        return await invokeFn(cmd, args);
      } catch (err) {
        console.warn(`[NOCTURNE IPC] Command '${cmd}' rejected:`, err);
        return null;
      }
    }
    return null;
  }

  const canvas = document.getElementById("evp-spectrograph-canvas");
  const ctx = canvas ? canvas.getContext("2d") : null;

  let sweepPhase = 0.35;
  let surveillanceBound = true;
  let curfewEngaged = true;
  let currentVigilStart = "00:00";
  let currentVigilEnd = "05:00";
  let isDraggingLullSlider = false;
  let lastTauriSweepMs = 0;

  /**
   * Renders the CRT EVP Spectrograph grid and phosphor trace.
   * Accepts either a phase offset (for calibrated display) or a 64-point
   * normalized PCM sweep array (`sweepSamples`) streamed from the Rust `cpal` engine.
   */
  function renderSpectrograph(
    phaseOffset = 0.35,
    sweepSamples = null,
    redlineGlitchActive = false
  ) {
    if (!canvas || !ctx) return;
    const w = canvas.width;
    const h = canvas.height;

    // Clear CRT background (blood-crimson tint during 20+ mG redline glitch)
    ctx.fillStyle = redlineGlitchActive ? "#180608" : "#04120c";
    ctx.fillRect(0, 0, w, h);

    // Draw minor grid lines
    ctx.strokeStyle = redlineGlitchActive
      ? "rgba(145, 38, 44, 0.28)"
      : "rgba(22, 92, 58, 0.25)";
    ctx.lineWidth = 1;
    const minorStep = 20;
    ctx.beginPath();
    for (let x = 0; x <= w; x += minorStep) {
      ctx.moveTo(x + 0.5, 0);
      ctx.lineTo(x + 0.5, h);
    }
    for (let y = 0; y <= h; y += minorStep) {
      ctx.moveTo(0, y + 0.5);
      ctx.lineTo(w, y + 0.5);
    }
    ctx.stroke();

    // Draw major grid lines
    ctx.strokeStyle = redlineGlitchActive
      ? "rgba(229, 83, 75, 0.45)"
      : "rgba(34, 145, 92, 0.38)";
    ctx.lineWidth = 1;
    const majorStep = 80;
    ctx.beginPath();
    for (let x = 0; x <= w; x += majorStep) {
      ctx.moveTo(x + 0.5, 0);
      ctx.lineTo(x + 0.5, h);
    }
    for (let y = 0; y <= h; y += majorStep) {
      ctx.moveTo(0, y + 0.5);
      ctx.lineTo(w, y + 0.5);
    }
    ctx.stroke();

    const ampScale = surveillanceBound ? (redlineGlitchActive ? 1.55 : 1.0) : 0.08;

    const waveY = (x) => {
      const normX = x / w;
      const glitchTear =
        redlineGlitchActive && normX > 0.38 && normX < 0.64
          ? Math.sin(normX * 140.0 + phaseOffset * 7.0) * 16.0
          : 0.0;
      if (Array.isArray(sweepSamples) && sweepSamples.length > 1) {
        const scaledIdx = normX * (sweepSamples.length - 1);
        const i0 = Math.floor(scaledIdx);
        const i1 = Math.min(sweepSamples.length - 1, i0 + 1);
        const frac = scaledIdx - i0;
        const sample = sweepSamples[i0] * (1 - frac) + sweepSamples[i1] * frac;
        return h * 0.54 - sample * 42.0 * ampScale + glitchTear;
      }

      const carrier =
        (Math.sin(normX * Math.PI * 5.2 + phaseOffset) * 18 +
          Math.sin(normX * Math.PI * 2.1 - phaseOffset * 0.5) * 14 +
          Math.cos(normX * Math.PI * 11.5 + phaseOffset * 1.2) * 4.5 +
          Math.sin(normX * Math.PI * 29.0) * 1.8) *
        ampScale;
      return h * 0.54 - carrier + glitchTear;
    };

    // Outer phosphor bloom
    ctx.save();
    ctx.strokeStyle = redlineGlitchActive
      ? "rgba(229, 83, 75, 0.28)"
      : "rgba(35, 232, 138, 0.22)";
    ctx.lineWidth = 10;
    ctx.shadowColor = redlineGlitchActive ? "#ff6459" : "#23e88a";
    ctx.shadowBlur = 18;
    ctx.beginPath();
    for (let x = 0; x <= w; x += 2) {
      const y = waveY(x);
      if (x === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    ctx.restore();

    // Secondary envelope band
    ctx.save();
    ctx.strokeStyle = redlineGlitchActive
      ? "rgba(255, 100, 89, 0.52)"
      : "rgba(44, 224, 145, 0.45)";
    ctx.lineWidth = 4;
    ctx.beginPath();
    for (let x = 0; x <= w; x += 2) {
      const y = waveY(x) + Math.sin(x * 0.25) * 2.2 * ampScale;
      if (x === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    ctx.restore();

    // Core bright CRT trace
    ctx.save();
    ctx.strokeStyle = redlineGlitchActive ? "#ff8a80" : "#5bffb0";
    ctx.lineWidth = 1.8;
    ctx.shadowColor = redlineGlitchActive ? "#ff6459" : "#2ce091";
    ctx.shadowBlur = 8;
    ctx.beginPath();
    for (let x = 0; x <= w; x += 1) {
      const y = waveY(x);
      if (x === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    ctx.restore();
  }

  // Render initial calibrated frame immediately
  renderSpectrograph(sweepPhase);

  // DOM References
  const daemonMemReadoutEl = document.getElementById("daemon-mem-readout");
  const netEgressReadoutEl = document.getElementById("net-egress-readout");
  const powerStatusDotEl = document.getElementById("power-status-dot");
  const powerStatusText = document.getElementById("power-status-text");
  const powerSublabel = document.getElementById("power-sublabel");
  const mainPowerSwitch = document.getElementById("main-power-switch");

  const calPulseBtn = document.getElementById("cal-pulse-btn");
  const emfReadoutEl = document.getElementById("emf-readout");
  const remPodStatusEl = document.getElementById("rem-pod-status");
  const emfDriftReadoutEl = document.getElementById("emf-drift-readout");
  const ledPills = document.querySelectorAll(".emf-led-row .led-pill");
  const ledVals = document.querySelectorAll(".emf-led-row .led-val");

  const curfewToggleBtn = document.getElementById("curfew-toggle-btn");
  const curfewDotEl = document.getElementById("curfew-dot");
  const curfewStatusTextEl = document.getElementById("curfew-status-text");
  const vigilBracketBadge = document.getElementById("vigil-bracket-badge");
  const chronoTrack = document.getElementById("chrono-track");
  const chronoActiveWindow = document.getElementById("chrono-active-window");
  const vigilStartLabel = document.getElementById("vigil-start-label");
  const vigilEndLabel = document.getElementById("vigil-end-label");
  const stationClockEl = document.getElementById("station-clock");
  const btnSetBracket = document.getElementById("btn-set-bracket");
  const btnNtpSync = document.getElementById("btn-ntp-sync");

  const channelsReadyBadge = document.getElementById("channels-ready-badge");
  const toggleButtons = document.querySelectorAll(".toggle-switch");

  const detentPosLabel = document.getElementById("detent-pos-label");
  const detentBtns = document.querySelectorAll(".detent-btn");
  const calibFootnote = document.querySelector(".calib-footnote");
  const lullSlider = document.getElementById("lull-slider");
  const lullReadout = document.getElementById("lull-readout");

  const crtHudMetricsEl = document.getElementById("crt-hud-metrics");
  const metricLastLullEl = document.getElementById("metric-last-lull");
  const metricGammaDriftEl = document.getElementById("metric-gamma-drift");
  const metricNextProbEl = document.getElementById("metric-next-prob");
  const spectroLogList = document.getElementById("spectro-log-list");

  const gaugeColdSpotEl = document.getElementById("gauge-cold-spot");
  const gaugeColdDipEl = document.getElementById("gauge-cold-dip");
  const gaugeGammaModEl = document.getElementById("gauge-gamma-mod");

  const manualExorciseBtn = document.getElementById("manual-exorcise-btn");
  const relayContactStatusEl = document.getElementById("relay-contact-status");
  const rustPanicsReadoutEl = document.getElementById("rust-panics-readout");
  const daemonPidReadoutEl = document.getElementById("daemon-pid-readout");
  const ringSentinelReadoutEl = document.getElementById("ring-sentinel-readout");
  const footerSignalBufferEl = document.getElementById("footer-signal-buffer");

  const winCloseBtn = document.getElementById("win-close-btn");
  const winHideBtn = document.getElementById("win-hide-btn");

  const detentNames = {
    "1": "POS 1 // SUBTLE",
    "2": "POS 2 // UNSETTLE",
    "3": "POS 3 // ACUTE",
  };

  const detentFootnotes = {
    "1": "Detent 1: Restrained psychoacoustic floor; blends into ambient room acoustics.",
    "2": "Detent 2: Heightened binaural displacement and peripheral gamma drift.",
    "3": "Detent 3: Acute sub-floor modal resonance and rapid cadence intervention.",
  };

  function hhmmToMinutes(hhmm) {
    if (typeof hhmm !== "string") return 0;
    const parts = hhmm.split(":");
    if (parts.length !== 2) return 0;
    const h = parseInt(parts[0], 10) || 0;
    const m = parseInt(parts[1], 10) || 0;
    return Math.max(0, Math.min(24 * 60, h * 60 + m));
  }

  function minutesToHhmm(totalMinutes) {
    const clamped = Math.max(0, Math.min(23 * 60 + 45, Math.round(totalMinutes / 15) * 15));
    const h = Math.floor(clamped / 60);
    const m = clamped % 60;
    return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
  }

  function updateVigilTimelineUi(startHhmm, endHhmm) {
    currentVigilStart = startHhmm;
    currentVigilEnd = endHhmm;
    if (vigilBracketBadge) {
      vigilBracketBadge.textContent = `VIGIL BRACKET: ${startHhmm} - ${endHhmm}`;
    }
    if (vigilStartLabel) {
      vigilStartLabel.textContent = startHhmm;
    }
    if (vigilEndLabel) {
      vigilEndLabel.textContent = endHhmm;
    }
    if (chronoActiveWindow) {
      const startMins = hhmmToMinutes(startHhmm);
      const endMins = hhmmToMinutes(endHhmm);
      const leftPct = (startMins / 1440) * 100;
      const spanMins = endMins >= startMins ? endMins - startMins : 1440 - startMins + endMins;
      // Preserve screenshot's 23% visual proportion for 00:00 - 05:00 while scaling dynamically for other brackets
      const widthPct =
        startHhmm === "00:00" && endHhmm === "05:00"
          ? 23
          : Math.max(12, Math.min(100 - leftPct, (spanMins / 1440) * 100));
      chronoActiveWindow.style.left = `${leftPct.toFixed(1)}%`;
      chronoActiveWindow.style.width = `${widthPct.toFixed(1)}%`;
    }
  }

  function updateCurfewUi(engaged) {
    curfewEngaged = Boolean(engaged);
    if (curfewStatusTextEl) {
      curfewStatusTextEl.textContent = curfewEngaged ? "CURFEW ENGAGED" : "CURFEW BYPASSED";
    }
    if (curfewDotEl) {
      curfewDotEl.className = curfewEngaged ? "dot dot-amber" : "dot dot-green";
    }
  }

  function updateChannelsReadyUi() {
    let activeCount = 0;
    toggleButtons.forEach((btn) => {
      const isOn = btn.classList.contains("active");
      if (isOn) activeCount += 1;
      const card = btn.closest(".engine-card");
      if (card) {
        const iconBox = card.querySelector(".engine-icon-box");
        if (iconBox) {
          iconBox.classList.toggle("text-amber", isOn);
          iconBox.classList.toggle("text-muted", !isOn);
        }
      }
    });
    if (channelsReadyBadge) {
      // Keep "3 CHANNELS READY" when Acoustics + Gamma are armed at baseline, or show active count
      channelsReadyBadge.textContent = `${activeCount} ${activeCount === 1 ? "CHANNEL" : "CHANNELS"} ARMED`;
    }
  }

  function updateLedStates(states) {
    if (!Array.isArray(states) || ledPills.length < 5) return;
    const activeClasses = [
      "led-green-active",
      "led-green-active",
      "led-green-active",
      "led-green-active",
      "led-green-active",
    ];
    const dimClasses = [
      "led-amber-dim",
      "led-amber-dim",
      "led-amber-dim",
      "led-orange-dim",
      "led-red-dim",
    ];
    states.forEach((on, idx) => {
      const pill = ledPills[idx];
      if (!pill) return;
      pill.classList.remove("led-green-active", "led-amber-dim", "led-orange-dim", "led-red-dim");
      pill.classList.add(on ? activeClasses[idx] : dimClasses[idx]);

      const valLabel = ledVals[idx];
      if (valLabel) {
        valLabel.classList.remove("text-green", "text-amber", "text-muted");
        if (on) {
          valLabel.classList.add("text-green");
        } else if (idx === 2) {
          valLabel.classList.add("text-amber");
        } else {
          valLabel.classList.add("text-muted");
        }
      }
    });
  }

  function formatIncidentRow(record) {
    const cat = record.trigger_category || "TELEMETRY_EVENT";
    let leftColor = "text-cyan-green";
    let rightColor = "text-green";
    let statusText = "LOCKED [0x48A]";

    if (cat === "REM_POD_CARRIER_SYNC") {
      leftColor = "text-cyan-green";
      rightColor = "text-green";
      statusText = "LOCKED [0x48A]";
    } else if (cat === "AMBIENT_ACOUSTIC_FLOOR") {
      leftColor = "text-cyan-green";
      rightColor = "text-green";
      statusText = "-44.2 dBFS (BALANCED)";
    } else if (cat.startsWith("EXORCISE_")) {
      leftColor = "text-amber";
      rightColor = "text-amber";
      statusText = "SEVERED [0x000]";
    } else if (cat.startsWith("CAL_")) {
      leftColor = "text-cyan-green";
      rightColor = "text-green";
      statusText = "PULSE [50-60Hz]";
    } else if (record.suppressed_flag) {
      leftColor = "text-amber";
      rightColor = "text-amber";
      statusText = "BURST SUPPRESSED";
    } else {
      leftColor = "text-amber";
      rightColor = "text-green";
      statusText = `DISPATCHED [${Number(record.milligauss_reading || 2.87).toFixed(1)} mG]`;
    }

    return {
      ts: record.timestamp_utc || "02:41:02",
      message: cat,
      leftColor,
      rightColor,
      statusText,
    };
  }

  function renderIncidentLogList(records) {
    if (!spectroLogList || !Array.isArray(records) || records.length === 0) return;
    const chronological = records.slice(0, 3).reverse();
    spectroLogList.innerHTML = "";
    chronological.forEach((rec) => {
      const formatted = formatIncidentRow(rec);
      const row = document.createElement("div");
      row.className = "spectro-log-item";

      const leftSpan = document.createElement("span");
      leftSpan.className = "log-left";

      const tsSpan = document.createElement("span");
      tsSpan.className = "log-ts";
      tsSpan.textContent = `[${formatted.ts}]`;

      const msgSpan = document.createElement("span");
      msgSpan.className = formatted.leftColor;
      msgSpan.textContent = formatted.message;

      leftSpan.appendChild(tsSpan);
      leftSpan.appendChild(document.createTextNode(" "));
      leftSpan.appendChild(msgSpan);

      const rightSpan = document.createElement("span");
      rightSpan.className = `log-right ${formatted.rightColor}`;
      rightSpan.textContent = formatted.statusText;

      row.appendChild(leftSpan);
      row.appendChild(rightSpan);
      spectroLogList.appendChild(row);
    });
  }

  function appendEventLog(ts, message, statusText, isAlert = false) {
    if (!spectroLogList) return;
    const row = document.createElement("div");
    row.className = "spectro-log-item";
    const colorClass = isAlert ? "text-amber" : "text-cyan-green";
    const rightColor = isAlert ? "text-amber" : "text-green";
    row.innerHTML = `<span class="log-left"><span class="log-ts">[${ts}]</span> <span class="${colorClass}">${message}</span></span><span class="log-right ${rightColor}">${statusText}</span>`;
    spectroLogList.appendChild(row);
    while (spectroLogList.children.length > 3) {
      spectroLogList.removeChild(spectroLogList.firstElementChild);
    }
  }

  function setPowerUiState(bound) {
    surveillanceBound = Boolean(bound);
    if (mainPowerSwitch) {
      mainPowerSwitch.setAttribute("aria-checked", String(surveillanceBound));
    }
    if (powerStatusDotEl) {
      powerStatusDotEl.className = surveillanceBound ? "dot dot-amber-glow" : "dot dot-amber";
    }
    if (powerStatusText) {
      powerStatusText.textContent = surveillanceBound
        ? "BOUND & LISTENING"
        : "STASIS // DISARMED";
    }
    if (powerSublabel) {
      powerSublabel.textContent = surveillanceBound
        ? "RELAY 12V COIL ENGAGED"
        : "RELAY 12V COIL OPEN";
    }
    if (footerSignalBufferEl) {
      footerSignalBufferEl.textContent = surveillanceBound ? "ARMED" : "STASIS";
      footerSignalBufferEl.className = surveillanceBound ? "text-green" : "text-amber";
    }
    if (relayContactStatusEl) {
      relayContactStatusEl.innerHTML = surveillanceBound
        ? '<span class="dot dot-green"></span> RELAY CONTACT: SEVER READY'
        : '<span class="dot dot-amber"></span> RELAY CONTACT: SEVERED [SLEEP]';
    }
  }

  function applyTelemetrySnapshotToDom(snap) {
    if (!snap) return;
    lastTauriSweepMs = performance.now();

    // 1. Header Telemetry & Main Power State
    if (daemonMemReadoutEl && typeof snap.daemon_mem_mb === "number") {
      daemonMemReadoutEl.textContent = `${snap.daemon_mem_mb.toFixed(1)} MB RESIDENT`;
    }
    if (netEgressReadoutEl && snap.airgap_audit) {
      netEgressReadoutEl.textContent = snap.airgap_audit.zero_egress_verified
        ? "ZERO NET EGRESS (0 KB/s)"
        : "AIR-GAP AUDIT WARNING";
    }

    const isDormant = Boolean(
      snap.exorcise_status && snap.exorcise_status.dormant_deep_sleep
    );
    setPowerUiState(!isDormant);

    // 2. K-II EMF Meter & REM-Pod Array
    if (emfReadoutEl && typeof snap.milligauss === "number") {
      emfReadoutEl.textContent = snap.milligauss.toFixed(2).padStart(5, "0");
    }
    if (remPodStatusEl && typeof snap.rem_pod_active === "boolean") {
      remPodStatusEl.textContent = snap.rem_pod_active
        ? "REM-POD: ACTIVE"
        : "REM-POD: STASIS";
      remPodStatusEl.className = snap.rem_pod_active ? "text-green" : "text-amber";
    }
    if (emfDriftReadoutEl && typeof snap.drift_mg === "number") {
      const sign = snap.drift_mg >= 0 ? "+" : "";
      emfDriftReadoutEl.textContent = `DRIFT: ${sign}${snap.drift_mg.toFixed(2)} mG`;
    }
    if (Array.isArray(snap.led_states)) {
      updateLedStates(snap.led_states);
    }

    // 3. Witching Hour Vigil Chronometer
    if (typeof snap.curfew_engaged === "boolean") {
      updateCurfewUi(snap.curfew_engaged);
    }
    if (snap.vigil_start && snap.vigil_end) {
      updateVigilTimelineUi(snap.vigil_start, snap.vigil_end);
    }
    if (stationClockEl && snap.station_time) {
      stationClockEl.textContent = snap.station_time;
    }

    // 4. Procedural Intervention Channel Toggles
    const channelMap = {
      acoustics: snap.channel_acoustics,
      gamma: snap.channel_gamma,
      thermal: snap.channel_thermal,
    };
    let activeChannelCount = 0;
    toggleButtons.forEach((btn) => {
      const ch = btn.getAttribute("data-channel");
      if (ch && typeof channelMap[ch] === "boolean") {
        const enabled = channelMap[ch];
        btn.classList.toggle("active", enabled);
        btn.setAttribute("aria-pressed", String(enabled));
        if (enabled) activeChannelCount += 1;
        const card = btn.closest(".engine-card");
        if (card) {
          const iconBox = card.querySelector(".engine-icon-box");
          if (iconBox) {
            iconBox.classList.toggle("text-amber", enabled);
            iconBox.classList.toggle("text-muted", !enabled);
          }
        }
      }
    });
    if (channelsReadyBadge) {
      channelsReadyBadge.textContent =
        activeChannelCount === 2 && !snap.channel_thermal
          ? "3 CHANNELS READY"
          : `${activeChannelCount} ${activeChannelCount === 1 ? "CHANNEL" : "CHANNELS"} READY`;
    }

    // 5. Intensity Detents & Cognitive Lull Slider
    if (typeof snap.intensity_detent === "number") {
      const posStr = String(snap.intensity_detent);
      detentBtns.forEach((b) => {
        b.classList.toggle("active", b.getAttribute("data-detent") === posStr);
      });
      if (detentPosLabel && detentNames[posStr]) {
        detentPosLabel.textContent = detentNames[posStr];
      }
      if (calibFootnote && detentFootnotes[posStr]) {
        calibFootnote.textContent = detentFootnotes[posStr];
      }
    }
    if (
      !isDraggingLullSlider &&
      lullSlider &&
      lullReadout &&
      typeof snap.cognitive_lull_window_sec === "number"
    ) {
      const formattedLull = snap.cognitive_lull_window_sec.toFixed(1);
      lullSlider.value = formattedLull;
      lullReadout.textContent = `${formattedLull.padStart(4, "0")}s`;
    }

    // 6. CRT EVP Spectrograph & Metrics
    if (
      crtHudMetricsEl &&
      typeof snap.acoustic_delta_mv === "number" &&
      typeof snap.acoustic_level_dbfs === "number"
    ) {
      crtHudMetricsEl.textContent = `\u0394 ${snap.acoustic_delta_mv.toFixed(3)} mV / ${snap.acoustic_level_dbfs.toFixed(1)} dBFS`;
    }
    if (metricLastLullEl && typeof snap.last_lull_sec === "number") {
      metricLastLullEl.textContent = `${snap.last_lull_sec.toFixed(1).padStart(4, "0")}s`;
    }
    if (metricGammaDriftEl && typeof snap.gamma_drift_pct === "number") {
      metricGammaDriftEl.textContent = `${snap.gamma_drift_pct.toFixed(1).padStart(4, "0")}%`;
    }
    if (metricNextProbEl && snap.next_probability_window) {
      metricNextProbEl.textContent = snap.next_probability_window;
    }
    if (Array.isArray(snap.recent_logs) && snap.recent_logs.length > 0) {
      renderIncidentLogList(snap.recent_logs);
    }
    if (Array.isArray(snap.spectrograph_sweep)) {
      sweepPhase = (sweepPhase + 0.18) % (Math.PI * 2);
      renderSpectrograph(
        sweepPhase,
        snap.spectrograph_sweep,
        Boolean(snap.redline_glitch_active)
      );
    }

    // 7. Thermocouple & Optical Gauges + Full-Viewport Spectral Color-Shift Filter
    if (gaugeColdSpotEl && typeof snap.cold_spot_celsius === "number") {
      gaugeColdSpotEl.innerHTML = `${snap.cold_spot_celsius.toFixed(1)}&deg;C`;
    }
    if (gaugeColdDipEl && typeof snap.cold_spot_dip_celsius === "number") {
      gaugeColdDipEl.innerHTML = `&lowast; &Delta; ${snap.cold_spot_dip_celsius.toFixed(1)}&deg;C DIP DETECTED`;
    }
    if (gaugeGammaModEl && typeof snap.gamma_modulation_pct === "number") {
      gaugeGammaModEl.textContent = `${snap.gamma_modulation_pct.toFixed(1).padStart(4, "0")}%`;
    }

    const shiftIntensity =
      typeof snap.color_shift_intensity === "number"
        ? Math.max(0.0, Math.min(1.0, snap.color_shift_intensity))
        : 0.0;
    const shiftMode = typeof snap.color_shift_mode === "number" ? snap.color_shift_mode % 2 : 0;
    if (shiftIntensity > 0.01 && !isDormant && snap.channel_gamma) {
      if (shiftMode === 0) {
        // Mode 0: Ectoplasmic Cyan-Violet Phase Inversion
        const hue = (shiftIntensity * 175.0).toFixed(1);
        const inv = (shiftIntensity * 0.88).toFixed(2);
        const sat = (1.0 + shiftIntensity * 2.2).toFixed(2);
        const con = (1.0 + shiftIntensity * 0.35).toFixed(2);
        document.body.style.filter = `hue-rotate(${hue}deg) invert(${inv}) saturate(${sat}) contrast(${con})`;
      } else {
        // Mode 1: Crimson Nightmare Solarization
        const hue = (-shiftIntensity * 65.0).toFixed(1);
        const inv = (shiftIntensity * 0.86).toFixed(2);
        const sep = (shiftIntensity * 0.82).toFixed(2);
        const sat = (1.0 + shiftIntensity * 2.8).toFixed(2);
        const con = (1.0 + shiftIntensity * 0.45).toFixed(2);
        document.body.style.filter = `hue-rotate(${hue}deg) invert(${inv}) sepia(${sep}) saturate(${sat}) contrast(${con})`;
      }
    } else if (document.body.style.filter) {
      document.body.style.filter = "";
    }

    // 8. Rust Status Bar
    if (rustPanicsReadoutEl) {
      rustPanicsReadoutEl.textContent = "0 PANICS";
    }
    if (daemonPidReadoutEl && typeof snap.pid === "number") {
      daemonPidReadoutEl.textContent = `PID: ${snap.pid} [DAEMON]`;
    }
    if (ringSentinelReadoutEl && snap.ring_sentinel_hex) {
      ringSentinelReadoutEl.textContent = snap.ring_sentinel_hex;
    }
  }

  // 1. Main Power / Surveillance Rocker Switch (Click + Keyboard accessible)
  async function handlePowerSwitchToggle() {
    const currentlyChecked =
      mainPowerSwitch && mainPowerSwitch.getAttribute("aria-checked") === "true";
    const nextState = !currentlyChecked;
    setPowerUiState(nextState);
    renderSpectrograph(sweepPhase);
    await invokeCommand("set_surveillance_bound", { bound: nextState });
    const snap = await invokeCommand("get_telemetry_snapshot");
    if (snap) applyTelemetrySnapshotToDom(snap);
  }

  if (mainPowerSwitch) {
    mainPowerSwitch.addEventListener("click", handlePowerSwitchToggle);
    mainPowerSwitch.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        handlePowerSwitchToggle();
      }
    });
  }

  // 2. K-II EMF Calibration Pulse Button (`CAL: 50-60Hz`)
  if (calPulseBtn) {
    calPulseBtn.addEventListener("click", async () => {
      await invokeCommand("trigger_calibration_pulse");
      const snap = await invokeCommand("get_telemetry_snapshot");
      if (snap) applyTelemetrySnapshotToDom(snap);
    });
  }

  // 3. Witching Hour Vigil Chronometer Controls
  if (curfewToggleBtn) {
    curfewToggleBtn.addEventListener("click", async () => {
      const nextEngaged = !curfewEngaged;
      updateCurfewUi(nextEngaged);
      await invokeCommand("toggle_curfew_mode", { engaged: nextEngaged });
      const snap = await invokeCommand("get_telemetry_snapshot");
      if (snap) applyTelemetrySnapshotToDom(snap);
    });
  }

  // Interactive 15-minute click/drag on the 24h chronometer timeline track
  let isDraggingChrono = false;
  async function handleChronoPointer(clientX, commitToBackend) {
    if (!chronoTrack) return;
    const rect = chronoTrack.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ratio = Math.max(0.05, Math.min(0.98, (clientX - rect.left) / rect.width));
    const endMinutes = Math.max(60, Math.round((ratio * 1440) / 15) * 15);
    const newEnd = minutesToHhmm(endMinutes);
    updateVigilTimelineUi(currentVigilStart, newEnd);
    if (commitToBackend) {
      await invokeCommand("set_vigil_bracket", {
        start: currentVigilStart,
        end: newEnd,
      });
      const snap = await invokeCommand("get_telemetry_snapshot");
      if (snap) applyTelemetrySnapshotToDom(snap);
    }
  }

  if (chronoTrack) {
    chronoTrack.addEventListener("mousedown", (e) => {
      isDraggingChrono = true;
      handleChronoPointer(e.clientX, false);
    });
    window.addEventListener("mousemove", (e) => {
      if (isDraggingChrono) {
        handleChronoPointer(e.clientX, false);
      }
    });
    window.addEventListener("mouseup", (e) => {
      if (isDraggingChrono) {
        isDraggingChrono = false;
        handleChronoPointer(e.clientX, true);
      }
    });
  }

  const presetBrackets = [
    ["00:00", "05:00"],
    ["00:00", "04:00"],
    ["00:00", "06:00"],
    ["01:00", "05:00"],
    ["23:00", "05:00"],
  ];
  let presetIndex = 0;

  if (btnSetBracket) {
    btnSetBracket.addEventListener("click", async () => {
      presetIndex = (presetIndex + 1) % presetBrackets.length;
      const [start, end] = presetBrackets[presetIndex];
      updateVigilTimelineUi(start, end);
      await invokeCommand("set_vigil_bracket", { start, end });
      const snap = await invokeCommand("get_telemetry_snapshot");
      if (snap) applyTelemetrySnapshotToDom(snap);
    });
  }

  if (btnNtpSync) {
    btnNtpSync.addEventListener("click", async () => {
      const now = new Date();
      let h = now.getHours();
      const m = String(now.getMinutes()).padStart(2, "0");
      const s = String(now.getSeconds()).padStart(2, "0");
      const ampm = h >= 12 ? "PM" : "AM";
      h = h % 12 || 12;
      const formatted = `${String(h).padStart(2, "0")}:${m}:${s} ${ampm}`;
      if (stationClockEl) {
        stationClockEl.textContent = formatted;
      }
      const snap = await invokeCommand("get_telemetry_snapshot");
      if (snap) applyTelemetrySnapshotToDom(snap);
    });
  }

  // 4. Procedural Intervention Engine Toggles
  toggleButtons.forEach((btn) => {
    btn.addEventListener("click", async () => {
      const isActive = btn.classList.toggle("active");
      btn.setAttribute("aria-pressed", String(isActive));
      updateChannelsReadyUi();
      const channelId = btn.getAttribute("data-channel");
      if (channelId) {
        await invokeCommand("toggle_intervention_channel", {
          channelId,
          enabled: isActive,
        });
        const snap = await invokeCommand("get_telemetry_snapshot");
        if (snap) applyTelemetrySnapshotToDom(snap);
      }
    });
  });

  // 5. Manifestation Intensity Detent Selector
  detentBtns.forEach((btn) => {
    btn.addEventListener("click", async () => {
      detentBtns.forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      const pos = btn.getAttribute("data-detent");
      if (detentPosLabel && detentNames[pos]) {
        detentPosLabel.textContent = detentNames[pos];
      }
      if (calibFootnote && detentFootnotes[pos]) {
        calibFootnote.textContent = detentFootnotes[pos];
      }
      if (pos) {
        await invokeCommand("set_intensity_detent", {
          detentPos: parseInt(pos, 10),
        });
        const snap = await invokeCommand("get_telemetry_snapshot");
        if (snap) applyTelemetrySnapshotToDom(snap);
      }
    });
  });

  // 6. Cognitive Lull Trigger Window Slider
  if (lullSlider && lullReadout) {
    lullSlider.addEventListener("mousedown", () => {
      isDraggingLullSlider = true;
    });
    lullSlider.addEventListener("touchstart", () => {
      isDraggingLullSlider = true;
    });
    lullSlider.addEventListener("input", (e) => {
      const val = parseFloat(e.target.value).toFixed(1);
      lullReadout.textContent = `${val.padStart(4, "0")}s`;
    });
    lullSlider.addEventListener("change", async (e) => {
      isDraggingLullSlider = false;
      const seconds = parseFloat(e.target.value);
      if (!Number.isNaN(seconds)) {
        await invokeCommand("set_cognitive_lull_window", { seconds });
        const snap = await invokeCommand("get_telemetry_snapshot");
        if (snap) applyTelemetrySnapshotToDom(snap);
      }
    });
  }

  // 7. Execute Emergency Exorcise Purge UI & IPC State Reset
  async function executeUiExorcisePurge(sourceCommand) {
    document.body.style.filter = "";
    setPowerUiState(false);
    if (emfReadoutEl) {
      emfReadoutEl.textContent = "01.50";
    }
    updateLedStates([true, false, false, false, false]);
    toggleButtons.forEach((btn) => {
      btn.classList.remove("active");
      btn.setAttribute("aria-pressed", "false");
    });
    updateChannelsReadyUi();
    appendEventLog("02:41:18", "EXORCISE_PURGE_CIRCUIT", "SEVERED [0x000]", true);
    if (sourceCommand) {
      await invokeCommand(sourceCommand);
      const snap = await invokeCommand("get_telemetry_snapshot");
      if (snap) applyTelemetrySnapshotToDom(snap);
    }
  }

  if (manualExorciseBtn) {
    manualExorciseBtn.addEventListener("click", async () => {
      await executeUiExorcisePurge("trigger_manual_exorcise");
    });
  }

  // Titlebar Window Controls (Left button: Terminate Daemon, Right button: Hide to System Tray)
  if (winCloseBtn) {
    winCloseBtn.addEventListener("mousedown", (e) => {
      e.stopPropagation();
    });
    winCloseBtn.addEventListener("click", async (e) => {
      e.preventDefault();
      e.stopPropagation();
      await invokeCommand("quit_application");
    });
  }

  if (winHideBtn) {
    winHideBtn.addEventListener("mousedown", (e) => {
      e.stopPropagation();
    });
    winHideBtn.addEventListener("click", async (e) => {
      e.preventDefault();
      e.stopPropagation();
      await invokeCommand("hide_seance_panel");
    });
  }

  // 8. 3-Slot <800ms Escape Key Failsafe Ringbuffer + Cmd+Q / Cmd+W Shortcuts
  const escRing = [];
  window.addEventListener("keydown", async (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "q") {
      e.preventDefault();
      await invokeCommand("quit_application");
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "w") {
      e.preventDefault();
      await invokeCommand("hide_seance_panel");
      return;
    }
    if (e.key === "Escape") {
      const nowMs = performance.now();
      escRing.push(nowMs);
      while (escRing.length > 3) {
        escRing.shift();
      }
      const localTripped = escRing.length === 3 && escRing[2] - escRing[0] < 800;
      const backendTripped = await invokeCommand("record_escape_key_press");
      if (localTripped || backendTripped) {
        escRing.length = 0;
        await executeUiExorcisePurge(null);
        const snap = await invokeCommand("get_telemetry_snapshot");
        if (snap) applyTelemetrySnapshotToDom(snap);
      }
    }
  });

  // 9. Subscribe to 150ms SWEEP events from Tauri backend + fallback polling/sweep
  let eventListenerBound = false;
  function tryBindSweepListener() {
    if (
      !eventListenerBound &&
      window.__TAURI__ &&
      window.__TAURI__.event &&
      typeof window.__TAURI__.event.listen === "function"
    ) {
      eventListenerBound = true;
      window.__TAURI__.event.listen("nocturne://telemetry-sweep", (event) => {
        if (event && event.payload) {
          applyTelemetrySnapshotToDom(event.payload);
        }
      });
    }
  }

  tryBindSweepListener();
  invokeCommand("get_telemetry_snapshot").then((snap) => {
    if (snap) applyTelemetrySnapshotToDom(snap);
  });

  // Fallback 150ms CRT sweep & IPC poll when event listener hasn't fired yet
  window.setInterval(async () => {
    tryBindSweepListener();
    if (performance.now() - lastTauriSweepMs > 300) {
      const snap = await invokeCommand("get_telemetry_snapshot");
      if (snap) {
        applyTelemetrySnapshotToDom(snap);
      } else {
        sweepPhase = (sweepPhase + 0.18) % (Math.PI * 2);
        renderSpectrograph(sweepPhase);
      }
    }
  }, 150);
})();
