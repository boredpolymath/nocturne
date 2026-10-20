/* ==========================================================================
   NOCTURNE MK-IV — browser specimen synthesizer
   A small Web Audio re-creation of the daemon's six procedural manifestations.
   No samples. Nothing is downloaded. Every sound is made fresh, just for you.
   ========================================================================== */
(() => {
  "use strict";

  let ctx = null;
  let master, limiter, analyser, reverb, reverbSend, noise;
  let drone = null;
  const live = new Set();

  const rand = (a, b) => a + Math.random() * (b - a);
  const pick = (arr) => arr[(Math.random() * arr.length) | 0];

  function build() {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return false;
    ctx = new AC();

    // master → psychoacoustic limiter → analyser (CRT tap) → speakers
    master = ctx.createGain();
    master.gain.value = 0.85;
    limiter = ctx.createDynamicsCompressor();
    limiter.threshold.value = -14;
    limiter.knee.value = 6;
    limiter.ratio.value = 12;
    limiter.attack.value = 0.003;
    limiter.release.value = 0.25;
    analyser = ctx.createAnalyser();
    analyser.fftSize = 512;
    master.connect(limiter).connect(analyser).connect(ctx.destination);

    // A room that's a little too big for where you're sitting.
    reverb = ctx.createConvolver();
    reverb.buffer = impulse(3.2, 2.4);
    reverbSend = ctx.createGain();
    reverbSend.gain.value = 0.35;
    reverbSend.connect(reverb).connect(master);

    // Shared white-noise buffer.
    noise = ctx.createBuffer(1, ctx.sampleRate * 4, ctx.sampleRate);
    const d = noise.getChannelData(0);
    for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
    return true;
  }

  function impulse(seconds, decay) {
    const len = Math.floor(ctx.sampleRate * seconds);
    const buf = ctx.createBuffer(2, len, ctx.sampleRate);
    for (let c = 0; c < 2; c++) {
      const ch = buf.getChannelData(c);
      for (let i = 0; i < len; i++) ch[i] = (Math.random() * 2 - 1) * Math.pow(1 - i / len, decay);
    }
    return buf;
  }

  function noiseSource() {
    const s = ctx.createBufferSource();
    s.buffer = noise;
    s.loop = true;
    s.loopStart = rand(0, 3);
    return s;
  }

  function track(src, stopAt) {
    live.add(src);
    src.onended = () => live.delete(src);
    src.start();
    src.stop(stopAt);
    return src;
  }

  /**
   * Binaural placement: interaural level + time difference.
   * az: -1 (hard left) … +1 (hard right). Returns the input node.
   */
  function binaural(az, wet = 0.4) {
    const input = ctx.createGain();
    const split = ctx.createGain();
    const lG = ctx.createGain();
    const rG = ctx.createGain();
    const lD = ctx.createDelay(0.01);
    const rD = ctx.createDelay(0.01);
    const merge = ctx.createChannelMerger(2);
    const itd = 0.00066 * Math.abs(az); // ~0.66 ms max, head-sized
    lG.gain.value = Math.cos(((az + 1) * Math.PI) / 4);
    rG.gain.value = Math.sin(((az + 1) * Math.PI) / 4);
    lD.delayTime.value = az > 0 ? itd : 0;
    rD.delayTime.value = az < 0 ? itd : 0;
    input.connect(split);
    split.connect(lG).connect(lD).connect(merge, 0, 0);
    split.connect(rG).connect(rD).connect(merge, 0, 1);
    merge.connect(master);
    const send = ctx.createGain();
    send.gain.value = wet;
    input.connect(send).connect(reverbSend);
    return input;
  }

  function env(param, t, peak, a, hold, r) {
    param.cancelScheduledValues(t);
    param.setValueAtTime(0.0001, t);
    param.exponentialRampToValueAtTime(peak, t + a);
    param.setValueAtTime(peak, t + a + hold);
    param.exponentialRampToValueAtTime(0.0001, t + a + hold + r);
  }

  /* ---------- 01 · Spatial floorboard creak (stick-slip) ---------- */
  function creak({ thermal = 0.5 } = {}) {
    const t = ctx.currentTime + 0.02;
    const dur = rand(1.2, 1.9);
    const out = binaural(pick([-0.75, -0.55, 0.6, 0.8]), 0.55);
    const vca = ctx.createGain();
    env(vca.gain, t, 0.55, 0.12, dur - 0.5, 0.38);

    // Slip pulses: a low sawtooth whose rate stutters irregularly.
    const slip = ctx.createOscillator();
    slip.type = "sawtooth";
    let f = rand(18, 26);
    for (let k = 0; k <= 40; k++) {
      const tt = t + (dur * k) / 40;
      const phase = Math.sin((Math.PI * k) / 40);
      f = Math.max(9, 14 + phase * rand(28, 46) + rand(-6, 6));
      slip.frequency.setValueAtTime(f, tt);
    }

    // Wood body: three resonant modes. Hotter CPU → lower, groaning voice.
    const base = 340 * (1.12 - thermal * 0.3);
    const sum = ctx.createGain();
    [1, 2.31, 4.07].forEach((m, i) => {
      const bp = ctx.createBiquadFilter();
      bp.type = "bandpass";
      bp.frequency.setValueAtTime(base * m, t);
      bp.frequency.linearRampToValueAtTime(base * m * rand(0.9, 1.08), t + dur);
      bp.Q.value = 18 + i * 8;
      const g = ctx.createGain();
      g.gain.value = [1, 0.55, 0.3][i];
      slip.connect(bp).connect(g).connect(sum);
    });
    sum.connect(vca).connect(out);
    track(slip, t + dur + 0.1);
    return dur;
  }

  /* ---------- 02 · Phantom keystroke echo ---------- */
  function key() {
    const t = ctx.currentTime + 0.02;
    const out = binaural(rand(-0.6, 0.6), 0.3);

    const n = noiseSource();
    const bp = ctx.createBiquadFilter();
    bp.type = "bandpass";
    bp.frequency.value = 3200;
    bp.Q.value = 2.5;
    const ng = ctx.createGain();
    env(ng.gain, t, 0.9, 0.001, 0.004, 0.025);
    n.connect(bp).connect(ng).connect(out);
    track(n, t + 0.1);

    const o = ctx.createOscillator();
    o.type = "triangle";
    o.frequency.setValueAtTime(520, t + 0.012);
    o.frequency.exponentialRampToValueAtTime(420, t + 0.05);
    const og = ctx.createGain();
    og.gain.value = 0;
    env(og.gain, t + 0.012, 0.6, 0.002, 0.005, 0.07);
    o.connect(og).connect(out);
    track(o, t + 0.2);
    return 0.35;
  }

  /* ---------- 03 · 3-formant spirit-box EVP whisper ---------- */
  const VOWELS = [
    [800, 1150, 2900], // a
    [400, 2000, 2550], // e
    [300, 2300, 3000], // i
    [450, 800, 2830],  // o
    [325, 700, 2530],  // u
  ];
  function whisper() {
    const t = ctx.currentTime + 0.02;
    const syll = 1 / 5.5;
    const count = (rand(9, 15)) | 0;
    const dur = count * syll + 0.4;

    const panNode = ctx.createStereoPanner();
    const start = rand(-1, 1);
    const curve = new Float32Array(64);
    for (let i = 0; i < 64; i++) curve[i] = Math.sin(start * 1.6 + (i / 63) * Math.PI * 1.6);
    panNode.pan.setValueCurveAtTime(curve, t, dur);
    const wet = ctx.createGain();
    wet.gain.value = 0.8;
    panNode.connect(master);
    panNode.connect(wet).connect(reverbSend);

    const vca = ctx.createGain();
    env(vca.gain, t, 1.0, 0.15, dur - 0.55, 0.4);

    // 5.5 Hz syllabic modulation
    const syl = ctx.createGain();
    syl.gain.value = 0.5;
    const lfo = ctx.createOscillator();
    lfo.frequency.value = 5.5;
    const lfoDepth = ctx.createGain();
    lfoDepth.gain.value = 0.5;
    lfo.connect(lfoDepth).connect(syl.gain);
    track(lfo, t + dur);

    const src = noiseSource();
    const formants = [0, 1, 2].map((i) => {
      const bp = ctx.createBiquadFilter();
      bp.type = "bandpass";
      bp.Q.value = [9, 13, 16][i];
      const g = ctx.createGain();
      g.gain.value = [1.6, 1.0, 0.6][i];
      src.connect(bp).connect(g).connect(syl);
      return bp;
    });
    for (let s = 0; s < count; s++) {
      const v = pick(VOWELS);
      const tt = t + s * syll;
      formants.forEach((bp, i) => bp.frequency.linearRampToValueAtTime(v[i] * rand(0.92, 1.1), tt + syll * 0.6));
    }
    syl.connect(vca).connect(panNode);
    track(src, t + dur + 0.1);
    return dur;
  }

  /* ---------- 04 · Wall-stud double knock + reed relay ---------- */
  function knock() {
    const t = ctx.currentTime + 0.02;
    const az = pick([-0.85, 0.85, -0.4]);
    const out = binaural(az, 0.6);
    const rap = (tt, amp) => {
      const o = ctx.createOscillator();
      o.frequency.setValueAtTime(190, tt);
      o.frequency.exponentialRampToValueAtTime(145, tt + 0.01);
      o.frequency.exponentialRampToValueAtTime(92, tt + 0.16);
      const g = ctx.createGain();
      env(g.gain, tt, amp, 0.002, 0.01, 0.2);
      o.connect(g).connect(out);
      track(o, tt + 0.3);

      const n = noiseSource();
      const lp = ctx.createBiquadFilter();
      lp.type = "lowpass";
      lp.frequency.value = 700;
      const ng = ctx.createGain();
      env(ng.gain, tt, amp * 0.6, 0.001, 0.004, 0.05);
      n.connect(lp).connect(ng).connect(out);
      track(n, tt + 0.1);
    };
    rap(t, 1.0);
    rap(t + 0.165, 0.85);

    const tc = t + 0.165 + rand(0.45, 0.7);
    const c = ctx.createOscillator();
    c.type = "square";
    c.frequency.value = 2400;
    const cb = ctx.createBiquadFilter();
    cb.type = "bandpass";
    cb.frequency.value = 2400;
    cb.Q.value = 6;
    const cg = ctx.createGain();
    env(cg.gain, tc, 0.18, 0.0008, 0.002, 0.018);
    c.connect(cb).connect(cg).connect(binaural(-az * 0.3, 0.2));
    track(c, tc + 0.06);
    return tc - t + 0.2;
  }

  /* ---------- 05 · Redline mains hum + corona crackle ---------- */
  function hum() {
    const t = ctx.currentTime + 0.02;
    const dur = 3.4;
    const vca = ctx.createGain();
    env(vca.gain, t, 0.5, 0.6, dur - 1.6, 1.0);
    const out = binaural(rand(-0.3, 0.3), 0.25);
    vca.connect(out);
    [[55, 0.5], [110, 0.38], [165, 0.22], [220, 0.12], [275, 0.07]].forEach(([f, a]) => {
      const o = ctx.createOscillator();
      o.frequency.value = f * rand(0.998, 1.002);
      const g = ctx.createGain();
      g.gain.value = a;
      o.connect(g).connect(vca);
      track(o, t + dur + 0.1);
    });
    // corona crackle: sparse noise ticks above 1.8 kHz
    const n = noiseSource();
    const hp = ctx.createBiquadFilter();
    hp.type = "highpass";
    hp.frequency.value = 1800;
    const cg = ctx.createGain();
    cg.gain.value = 0;
    for (let tt = t + 0.4; tt < t + dur - 0.6; tt += rand(0.015, 0.14)) {
      cg.gain.setValueAtTime(rand(0.05, 0.35), tt);
      cg.gain.setValueAtTime(0, tt + rand(0.002, 0.008));
    }
    n.connect(hp).connect(cg).connect(out);
    track(n, t + dur + 0.1);
    return dur;
  }

  /* ---------- 06 · Infrasonic room breath ---------- */
  function breath() {
    const t = ctx.currentTime + 0.02;
    const dur = 6.5;
    const vca = ctx.createGain();
    vca.gain.setValueAtTime(0.0001, t);
    // two slow breaths
    [[0, 1.8, 1.4], [3.3, 1.8, 1.4]].forEach(([o, inn, ex]) => {
      vca.gain.exponentialRampToValueAtTime(0.9, t + o + inn);
      vca.gain.exponentialRampToValueAtTime(0.0001, t + o + inn + ex);
    });
    vca.connect(master);

    const n = noiseSource();
    const lp = ctx.createBiquadFilter();
    lp.type = "lowpass";
    lp.frequency.value = 160;
    lp.Q.value = 2;
    const ng = ctx.createGain();
    ng.gain.value = 1.4;
    n.connect(lp).connect(ng).connect(vca);
    track(n, t + dur);

    // 19 Hz carrier (felt, not heard) + 38 Hz so laptop speakers get something.
    [[19, 0.7], [38, 0.35]].forEach(([f, a]) => {
      const o = ctx.createOscillator();
      o.frequency.value = f;
      const g = ctx.createGain();
      g.gain.value = a;
      o.connect(g).connect(vca);
      track(o, t + dur);
    });
    return dur;
  }

  /* ---------- Ambient room tone ---------- */
  function startDrone() {
    if (drone) return;
    const t = ctx.currentTime;
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(0.05, t + 4);
    g.connect(master);

    const n = noiseSource();
    const lp = ctx.createBiquadFilter();
    lp.type = "lowpass";
    lp.frequency.value = 260;
    lp.Q.value = 4;
    const lfo = ctx.createOscillator();
    lfo.frequency.value = 0.07;
    const lfoG = ctx.createGain();
    lfoG.gain.value = 140;
    lfo.connect(lfoG).connect(lp.frequency);
    n.connect(lp).connect(g);

    const o1 = ctx.createOscillator();
    const o2 = ctx.createOscillator();
    o1.frequency.value = 55;
    o2.frequency.value = 55.37; // slow beating, like something breathing in the walls
    const og = ctx.createGain();
    og.gain.value = 0.5;
    o1.connect(og);
    o2.connect(og);
    og.connect(g);

    [n, lfo, o1, o2].forEach((s) => s.start());
    drone = { g, nodes: [n, lfo, o1, o2] };
  }

  const SYNTHS = { creak, key, whisper, knock, hum, breath };

  window.NocturneAudio = {
    get on() {
      return !!ctx && ctx.state === "running";
    },
    async wake() {
      if (!ctx && !build()) return false;
      if (ctx.state === "suspended") await ctx.resume();
      startDrone();
      return true;
    },
    async sleep() {
      if (ctx) await ctx.suspend();
    },
    /** Returns the duration in seconds, or 0 if audio is unavailable. */
    play(name, opts) {
      if (!ctx || ctx.state !== "running" || !SYNTHS[name]) return 0;
      return SYNTHS[name](opts);
    },
    /** Exorcise: sever everything immediately. */
    sever() {
      if (!ctx) return;
      live.forEach((s) => {
        try { s.stop(); } catch (_) { /* already gone */ }
      });
      live.clear();
      const c = ctx;
      ctx = null;
      drone = null;
      c.close();
    },
    /** 64-point time-domain tap for the CRT. Returns null if silent. */
    scope(out) {
      if (!ctx || ctx.state !== "running") return null;
      const buf = new Float32Array(analyser.fftSize);
      analyser.getFloatTimeDomainData(buf);
      const step = buf.length / out.length;
      for (let i = 0; i < out.length; i++) out[i] = buf[(i * step) | 0];
      return out;
    },
  };
})();
