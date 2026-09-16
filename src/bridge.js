// Long-lived bridge between Rust and the webview.
// Rust owns the API key and every OpenAI HTTP request; this script only runs the
// WebRTC peer connection (microphone, echo cancellation, playback) and animations.
const post = (message) => { try { dioxus.send(message); } catch (_) {} };

const config = { fakeMicrophone: false };
const live = { attempt: null, pc: null, dc: null, stream: null, track: null, audio: null, ctx: null, meter: null, muted: false };

function stopStream(stream) {
  if (stream) stream.getTracks().forEach((t) => { try { t.stop(); } catch (_) {} });
}

function teardown() {
  if (live.meter) clearInterval(live.meter);
  live.meter = null;
  if (live.dc) { live.dc.onmessage = null; live.dc.onclose = null; live.dc.onopen = null; try { live.dc.close(); } catch (_) {} }
  if (live.pc) { live.pc.ontrack = null; live.pc.oniceconnectionstatechange = null; try { live.pc.close(); } catch (_) {} }
  stopStream(live.stream);
  if (live.audio) { try { live.audio.pause(); } catch (_) {} live.audio.srcObject = null; }
  if (live.ctx) { try { live.ctx.close(); } catch (_) {} }
  Object.assign(live, { attempt: null, pc: null, dc: null, stream: null, track: null, ctx: null, muted: false });
  post({ kind: "levels", input: 0, output: 0 });
}

function analyser(ctx, stream) {
  try {
    const node = ctx.createAnalyser();
    node.fftSize = 512;
    ctx.createMediaStreamSource(stream).connect(node);
    const data = new Float32Array(node.fftSize);
    return () => {
      node.getFloatTimeDomainData(data);
      let sum = 0;
      for (let i = 0; i < data.length; i++) sum += data[i] * data[i];
      return Math.sqrt(sum / data.length);
    };
  } catch (_) {
    return () => 0;
  }
}

function waitForIce(pc, attempt) {
  // WebKit does not reliably fire icegatheringstatechange; poll like the iPhone client,
  // and also accept the end-of-candidates signal.
  return new Promise((resolve, reject) => {
    let finished = false;
    pc.addEventListener("icecandidate", (event) => { if (!event.candidate) finished = true; });
    const started = Date.now();
    const timer = setInterval(() => {
      if (live.attempt !== attempt) { clearInterval(timer); reject(new Error("cancelled")); return; }
      if (finished || pc.iceGatheringState === "complete") { clearInterval(timer); resolve(); return; }
      if (Date.now() - started > 10000) { clearInterval(timer); reject(new Error("timeout")); }
    }, 100);
  });
}

async function connect(attempt) {
  teardown();
  live.attempt = attempt;
  const fail = (code) => { if (live.attempt === attempt) post({ kind: "error", attempt, code }); };
  let stream;
  if (config.fakeMicrophone && window.RTCPeerConnection) {
    // Debug builds only: a quiet tone stands in for the microphone.
    const ctx = new AudioContext();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    gain.gain.value = 0.05;
    const dest = ctx.createMediaStreamDestination();
    osc.connect(gain).connect(dest);
    osc.start();
    stream = dest.stream;
  } else {
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia || !window.RTCPeerConnection) return fail("unsupported");
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true } });
    } catch (_) {
      return fail("microphone");
    }
  }
  if (live.attempt !== attempt) { stopStream(stream); return; }
  try {
    const pc = new RTCPeerConnection({ bundlePolicy: "max-bundle" });
    live.pc = pc; live.stream = stream; live.track = stream.getAudioTracks()[0];
    live.ctx = new (window.AudioContext || window.webkitAudioContext)();
    const inputLevel = analyser(live.ctx, stream);
    let outputLevel = () => 0;
    pc.addTrack(live.track, stream);
    pc.ontrack = (event) => {
      if (pc !== live.pc) return;
      const remote = event.streams[0] || new MediaStream([event.track]);
      if (!live.audio) { live.audio = new Audio(); live.audio.autoplay = true; }
      live.audio.srcObject = remote;
      live.audio.play().catch(() => {});
      outputLevel = analyser(live.ctx, remote);
    };
    const dc = pc.createDataChannel("oai-events", { ordered: true });
    live.dc = dc;
    dc.onopen = () => { if (pc === live.pc) post({ kind: "channel", attempt, open: true }); };
    dc.onclose = () => { if (pc === live.pc) post({ kind: "channel", attempt, open: false }); };
    dc.onmessage = (event) => { if (pc === live.pc && typeof event.data === "string") post({ kind: "event", attempt, data: event.data }); };
    pc.oniceconnectionstatechange = () => { if (pc === live.pc) post({ kind: "ice", attempt, state: pc.iceConnectionState }); };
    const offer = await pc.createOffer({ offerToReceiveAudio: true, offerToReceiveVideo: false });
    await pc.setLocalDescription(offer);
    await waitForIce(pc, attempt);
    if (live.attempt !== attempt) return;
    let lastIn = 0, lastOut = 0;
    live.meter = setInterval(() => {
      if (pc !== live.pc) return;
      lastIn = lastIn * 0.35 + Math.min(1, inputLevel() * 8) * 0.65;
      lastOut = lastOut * 0.35 + Math.min(1, outputLevel() * 8) * 0.65;
      post({ kind: "levels", input: live.muted ? 0 : lastIn, output: lastOut });
    }, 100);
    post({ kind: "offer", attempt, sdp: pc.localDescription.sdp });
  } catch (error) {
    fail(String(error && error.message) === "timeout" ? "timeout" : "connection");
  }
}

async function answer(attempt, sdp) {
  if (attempt !== live.attempt || !live.pc) return;
  try {
    await live.pc.setRemoteDescription({ type: "answer", sdp });
    if (live.ctx && live.ctx.state === "suspended") live.ctx.resume().catch(() => {});
  } catch (_) {
    post({ kind: "error", attempt, code: "connection" });
  }
}

function sendEvent(attempt, data) {
  if (attempt !== live.attempt || !live.dc || live.dc.readyState !== "open") return;
  try { live.dc.send(data); } catch (_) {}
}

function mute(muted) {
  live.muted = muted;
  if (live.track) live.track.enabled = !muted;
}

// ---- Orb animation: every .mural-orb reads data-energy / data-active from its element. ----
const reduceMotion = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function orbPath(side, phase, energy) {
  const pts = [];
  for (let i = 0; i < 12; i++) {
    const a = (i / 12) * Math.PI * 2;
    const wave = Math.sin(a * 3 + phase) * 0.021 + Math.cos(a * 2 - phase * 0.7) * (0.012 + energy * 0.025);
    const r = side * (0.47 + wave);
    pts.push([side / 2 + Math.cos(a) * r, side / 2 + Math.sin(a) * r]);
  }
  const mid = (p, q) => [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2];
  const start = mid(pts[11], pts[0]);
  let d = `M${start[0].toFixed(2)} ${start[1].toFixed(2)}`;
  for (let i = 0; i < 12; i++) {
    const m = mid(pts[i], pts[(i + 1) % 12]);
    d += ` Q${pts[i][0].toFixed(2)} ${pts[i][1].toFixed(2)} ${m[0].toFixed(2)} ${m[1].toFixed(2)}`;
  }
  return d + "Z";
}

const smoothed = new WeakMap();
function frame(now) {
  const t = reduceMotion ? 0 : now / 1000;
  document.querySelectorAll(".mural-orb").forEach((el) => {
    const active = el.dataset.active !== "false";
    const target = reduceMotion ? 0 : Math.max(0, Math.min(1, parseFloat(el.dataset.energy || "0") || 0));
    const prev = smoothed.get(el) ?? target;
    const energy = prev + (target - prev) * 0.25;
    smoothed.set(el, energy);
    if (!active && el.dataset.drawn) return;
    el.dataset.drawn = "1";
    const phase = t * 0.72;
    const clip = el.querySelector(".orb-clip-path");
    if (clip) clip.setAttribute("d", orbPath(100, phase, energy));
    const body = el.querySelector(".orb-body");
    if (body) {
      const rot = Math.sin(phase * 0.5) * 3;
      const lift = reduceMotion ? 0 : Math.sin(t * 0.9) * 4 - 5;
      body.style.transform = `translateY(${lift}px) rotate(${rot}deg) scale(${1 + energy * 0.045})`;
    }
    const glow = el.querySelector(".orb-hot");
    if (glow) {
      glow.setAttribute("cx", (50 + Math.sin(phase) * 8).toFixed(2));
      glow.setAttribute("cy", (50 + Math.cos(phase) * 6).toFixed(2));
    }
  });
  const bg = document.querySelector(".onboarding-glow");
  if (bg) {
    const p = reduceMotion ? 0 : (now / 1000) * 0.14;
    bg.style.setProperty("--gx", `${50 + Math.sin(p) * 12}%`);
    bg.style.setProperty("--gy", `${45 + Math.cos(p) * 10}%`);
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

post({ kind: "ready" });

while (true) {
  const command = await dioxus.recv();
  if (!command || typeof command !== "object") continue;
  switch (command.cmd) {
    case "connect": connect(command.attempt); break;
    case "answer": answer(command.attempt, command.sdp); break;
    case "send": sendEvent(command.attempt, command.data); break;
    case "mute": mute(!!command.muted); break;
    case "disconnect": teardown(); break;
    case "config": config.fakeMicrophone = !!command.fakeMicrophone; break;
  }
}
