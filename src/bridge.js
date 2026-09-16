// Long-lived bridge between Rust and the webview.
// Rust owns the API keys and every HTTP request; this script only runs the
// WebRTC peer connection (microphone, echo cancellation, playback), the local
// speech loop used with an Ollama teacher, and animations.
const post = (message) => { try { dioxus.send(message); } catch (_) {} };

const config = { fakeMicrophone: false, fakeSpeech: [], trace: false };
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
  localMute(muted);
}


// ---- Local voice: this Mac's speech recognition and synthesis, used with an Ollama teacher. ----
// Rust starts it, receives each finished utterance ("heard"), and sends back text to speak.
// Recognition pauses while Mural speaks so it never hears itself.
// Debug builds only: scripted phrases stand in for the learner's speech.
class FakeRecognition {
  start() {
    setTimeout(() => {
      const text = config.fakeSpeech.shift();
      if (text && this.onresult) this.onresult({ resultIndex: 0, results: [Object.assign([{ transcript: text }], { isFinal: true })] });
      if (this.onend) this.onend();
    }, 1500);
  }
  abort() {}
}
const NativeRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
const Recognition = function () { return config.fakeMicrophone ? new FakeRecognition() : new NativeRecognition(); };
const local = { attempt: null, lang: "en", recognition: null, speaking: 0, utterances: [], muted: false, stream: null, ctx: null, meter: null, voice: null, networkErrors: 0, quickEnds: 0, restart: null };

function pickVoice(lang) {
  if (!window.speechSynthesis) return null;
  const base = lang.split("-")[0].toLowerCase();
  const norm = (v) => v.lang.replace("_", "-").toLowerCase();
  const voices = speechSynthesis.getVoices().filter((v) => norm(v).split("-")[0] === base);
  const score = (v) => (norm(v) === lang.toLowerCase() ? 4 : 0) + (/premium/i.test(v.name + v.voiceURI) ? 2 : /enhanced/i.test(v.name + v.voiceURI) ? 1 : 0);
  return voices.sort((a, b) => score(b) - score(a))[0] || null;
}

function stopListening() {
  clearTimeout(local.restart);
  const r = local.recognition;
  local.recognition = null;
  if (r) { r.onresult = null; r.onerror = null; r.onend = null; try { r.abort(); } catch (_) {} }
}

function localTeardown() {
  stopListening();
  if (local.meter) clearInterval(local.meter);
  if (window.speechSynthesis) speechSynthesis.cancel();
  stopStream(local.stream);
  if (local.ctx) { try { local.ctx.close(); } catch (_) {} }
  Object.assign(local, { attempt: null, speaking: 0, utterances: [], muted: false, stream: null, ctx: null, meter: null, networkErrors: 0, quickEnds: 0 });
}

function localFail(attempt, code) {
  if (local.attempt !== attempt) return;
  localTeardown();
  post({ kind: "error", attempt, code });
}

const trace = (attempt, text) => { if (config.trace) post({ kind: "local", attempt, event: "trace", text }); };

function listen(attempt, delay = 0) {
  clearTimeout(local.restart);
  if (delay) { local.restart = setTimeout(() => listen(attempt), delay); return; }
  if (local.attempt !== attempt || local.speaking || local.muted || local.recognition) return;
  const r = new Recognition();
  r.lang = local.lang;
  r.continuous = false;
  r.interimResults = true;
  r.maxAlternatives = 1;
  local.recognition = r;
  // WebKit keeps sending interim results and may never mark one final, so a pause in the
  // transcript ends the turn: stop() asks for the final text, and the latest interim text
  // is used if none arrives.
  let heard = "";
  let latest = "";
  let pause = null;
  let stopTimer = null;
  const finish = () => {
    clearTimeout(pause);
    if (stopTimer) return;
    try { r.stop(); } catch (_) {}
    stopTimer = setTimeout(() => { if (local.recognition === r) { try { r.abort(); } catch (_) {} r.onend(); } }, 1500);
  };
  const cap = setTimeout(finish, 30000);
  const startedAt = Date.now();
  r.onresult = (event) => {
    let text = "";
    let final = true;
    for (let i = 0; i < event.results.length; i++) {
      text += event.results[i][0].transcript;
      if (!event.results[i].isFinal) final = false;
    }
    local.networkErrors = 0;
    local.quickEnds = 0;
    if (final) heard = text;
    const changed = text !== latest;
    latest = text;
    trace(attempt, `result final=${final} ${text.slice(-60)}`);
    if (final) { finish(); return; }
    if (changed) { clearTimeout(pause); pause = setTimeout(finish, 1500); }
  };
  r.onstart = () => trace(attempt, "start");
  r.onaudiostart = () => trace(attempt, "audiostart");
  r.onspeechstart = () => trace(attempt, "speechstart");
  r.onerror = (event) => {
    trace(attempt, `error ${event.error} ${event.message || ""}`);
    if (/disabled/i.test(event.message || "")) localFail(attempt, "speech-disabled");
    else if (event.error === "not-allowed" || event.error === "service-not-allowed") localFail(attempt, "speech");
    else if (event.error === "audio-capture") localFail(attempt, "microphone");
    else if (event.error === "network" && ++local.networkErrors >= 3) localFail(attempt, "speech-network");
    else if (event.error === "language-not-supported") localFail(attempt, "speech-unsupported");
  };
  r.onend = () => {
    trace(attempt, `end heard=${heard.length} latest=${latest.length}`);
    clearTimeout(pause); clearTimeout(stopTimer); clearTimeout(cap);
    if (local.recognition !== r) return;
    local.recognition = null;
    const text = (heard || latest).trim();
    // Rust answers a finished utterance with "speak" (or "listen" if there is nothing to say).
    if (text) { post({ kind: "local", attempt, event: "heard", text }); return; }
    // A recognizer that keeps ending at once is broken, not waiting for speech: say so instead of spinning.
    local.quickEnds = Date.now() - startedAt < 1000 ? local.quickEnds + 1 : 0;
    if (local.quickEnds >= 8) return localFail(attempt, "speech-unavailable");
    listen(attempt, local.networkErrors ? 1000 : 150 * Math.max(1, local.quickEnds));
  };
  try { r.start(); } catch (_) { local.recognition = null; listen(attempt, 500); }
}

function speak(attempt, text) {
  if (local.attempt !== attempt || !window.speechSynthesis) return;
  stopListening();
  const u = new SpeechSynthesisUtterance(text);
  u.lang = local.lang;
  const voice = local.voice || (local.voice = pickVoice(local.lang));
  if (voice) u.voice = voice;
  u.rate = 0.95;
  if (config.fakeMicrophone) u.volume = 0;
  local.speaking += 1;
  local.utterances.push(u); // WebKit can drop events for utterances that are garbage-collected.
  let finished = false;
  const done = () => {
    if (finished || local.attempt !== attempt) return;
    finished = true;
    local.utterances = local.utterances.filter((x) => x !== u);
    local.speaking = Math.max(0, local.speaking - 1);
    if (!local.speaking) listen(attempt, 350);
    trace(attempt, `speaking=${local.speaking} voice=${voice ? voice.name : "default"}`);
  };
  u.onstart = () => trace(attempt, "speak start");
  u.onend = () => { trace(attempt, "speak end"); done(); };
  u.onerror = (e) => { trace(attempt, `speak error ${e.error}`); done(); };
  speechSynthesis.speak(u);
}

async function localStart(attempt, lang) {
  teardown();
  localTeardown();
  local.attempt = attempt;
  local.lang = lang;
  if (!NativeRecognition || !window.speechSynthesis) return localFail(attempt, "speech-unsupported");
  let stream;
  if (config.fakeMicrophone) {
    const ctx = new AudioContext();
    const dest = ctx.createMediaStreamDestination();
    stream = dest.stream;
  } else {
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) return localFail(attempt, "unsupported");
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true } });
    } catch (_) {
      return localFail(attempt, "microphone");
    }
  }
  if (local.attempt !== attempt) { stopStream(stream); return; }
  local.stream = stream;
  local.ctx = new (window.AudioContext || window.webkitAudioContext)();
  const inputLevel = analyser(local.ctx, stream);
  local.voice = pickVoice(lang);
  speechSynthesis.onvoiceschanged = () => { local.voice = pickVoice(local.lang); };
  let lastIn = 0, lastOut = 0;
  local.meter = setInterval(() => {
    const speaking = local.speaking > 0 && speechSynthesis.speaking;
    const input = local.muted || local.speaking ? 0 : Math.min(1, inputLevel() * 8);
    // Speech synthesis has no audio stream to measure, so the orb gets a gentle pulse instead.
    const output = speaking ? 0.3 + 0.25 * Math.abs(Math.sin(Date.now() / 170)) : 0;
    lastIn = lastIn * 0.35 + input * 0.65;
    lastOut = lastOut * 0.35 + output * 0.65;
    post({ kind: "levels", input: lastIn, output: lastOut });
  }, 100);
  // Mural greets first; listening starts once the greeting has been spoken.
  post({ kind: "local", attempt, event: "ready" });
}

function localMute(muted) {
  local.muted = muted;
  if (!local.attempt) return;
  if (muted) stopListening();
  else listen(local.attempt);
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
    case "disconnect": teardown(); localTeardown(); break;
    case "local_start": localStart(command.attempt, command.lang || "en"); break;
    case "speak": speak(command.attempt, String(command.text || "")); break;
    case "listen": listen(command.attempt); break;
    case "config": Object.assign(config, { fakeMicrophone: !!command.fakeMicrophone, fakeSpeech: command.fakeSpeech || [], trace: !!command.trace }); break;
  }
}
