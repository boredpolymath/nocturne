/**
 * Project Nocturne (MK-IV) — Séance Panel Presentation Skeleton
 * Renders the CRT EVP Spectrograph Canvas grid & phosphor trace,
 * and provides lightweight UI skeleton bindings for Tauri IPC commands.
 */

(function initSeancePanel() {
  const canvas = document.getElementById("evp-spectrograph-canvas");
  if (canvas) {
    const ctx = canvas.getContext("2d");

    function renderSpectrograph(phaseOffset = 0) {
      const w = canvas.width;
      const h = canvas.height;

      // Clear CRT background
      ctx.fillStyle = "#04120c";
      ctx.fillRect(0, 0, w, h);

      // Draw minor grid lines
      ctx.strokeStyle = "rgba(22, 92, 58, 0.25)";
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
      ctx.strokeStyle = "rgba(34, 145, 92, 0.38)";
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

      // Composite phosphor wave function matching the CRT EVP Spectrograph screenshot
      const waveY = (x) => {
        const normX = x / w;
        const carrier =
          Math.sin(normX * Math.PI * 5.2 + phaseOffset) * 18 +
          Math.sin(normX * Math.PI * 2.1 - phaseOffset * 0.5) * 14 +
          Math.cos(normX * Math.PI * 11.5 + phaseOffset * 1.2) * 4.5 +
          Math.sin(normX * Math.PI * 29.0) * 1.8;
        return h * 0.54 - carrier;
      };

      // Outer phosphor bloom
      ctx.save();
      ctx.strokeStyle = "rgba(35, 232, 138, 0.22)";
      ctx.lineWidth = 10;
      ctx.shadowColor = "#23e88a";
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
      ctx.strokeStyle = "rgba(44, 224, 145, 0.45)";
      ctx.lineWidth = 4;
      ctx.beginPath();
      for (let x = 0; x <= w; x += 2) {
        const y = waveY(x) + Math.sin(x * 0.25) * 2.2;
        if (x === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
      ctx.restore();

      // Core bright CRT trace
      ctx.save();
      ctx.strokeStyle = "#5bffb0";
      ctx.lineWidth = 1.8;
      ctx.shadowColor = "#2ce091";
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

    // Render static calibrated frame immediately
    renderSpectrograph(0.35);
  }

  // Lightweight skeleton UI interactions (visual state only)
  const toggles = document.querySelectorAll(".toggle-switch");
  toggles.forEach((btn) => {
    btn.addEventListener("click", () => {
      const isActive = btn.classList.toggle("active");
      btn.setAttribute("aria-pressed", String(isActive));
    });
  });

  const detentBtns = document.querySelectorAll(".detent-btn");
  const detentPosLabel = document.getElementById("detent-pos-label");
  const detentNames = {
    "1": "POS 1 // SUBTLE",
    "2": "POS 2 // UNSETTLE",
    "3": "POS 3 // ACUTE",
  };
  detentBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      detentBtns.forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      const pos = btn.getAttribute("data-detent");
      if (detentPosLabel && detentNames[pos]) {
        detentPosLabel.textContent = detentNames[pos];
      }
    });
  });

  const lullSlider = document.getElementById("lull-slider");
  const lullReadout = document.getElementById("lull-readout");
  if (lullSlider && lullReadout) {
    lullSlider.addEventListener("input", (e) => {
      const val = parseFloat(e.target.value).toFixed(1);
      lullReadout.textContent = `${val.padStart(4, "0")}s`;
    });
  }
})();
