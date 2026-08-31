// Sound utility: Web Audio API based "ding" sound for task completion.
// This is the previously verified-working模式: create AudioContext at module
// load, resume on first gesture, play synchronously with zero await.

let _ctx: AudioContext | null = null;
let _master: GainNode | null = null;

function _ensureContext() {
    if (_ctx && _master) return;
    try {
        _ctx = new (window.AudioContext || (window as any).webkitAudioContext)();
        _master = _ctx.createGain();
        _master.gain.value = 1.4;
        _master.connect(_ctx.destination);
    } catch (e) {}
}

// Resume on first user interaction (browser autoplay policy)
let _resumeRegistered = false;
function _registerResume() {
    if (_resumeRegistered) return;
    _resumeRegistered = true;
    const handler = () => {
        _ensureContext();
        if (_ctx && _ctx.state === 'suspended') {
            _ctx.resume().catch(() => {});
        }
    };
    window.addEventListener('pointerdown', handler, { once: true });
}

// Create context and register resume at module load
_ensureContext();
_registerResume();

export function playCompleteSound() {
    try {
        _ensureContext();
        if (!_ctx || !_master) return;

        // If suspended, resume fire-and-forget (don't await), then play
        // immediately. The browser starts playing as soon as context resumes,
        // which is near-instant after a user gesture.
        if (_ctx.state === 'suspended') {
            _ctx.resume().catch(() => {});
        }

        const ctx = _ctx;
        const master = _master;
        const t = ctx.currentTime;

        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = 'triangle';
        osc.frequency.setValueAtTime(659, t);
        osc.frequency.exponentialRampToValueAtTime(1318, t + 0.18);
        // Start loud immediately — no fade-in
        gain.gain.setValueAtTime(0.3, t);
        gain.gain.exponentialRampToValueAtTime(0.001, t + 0.3);
        osc.connect(gain);
        gain.connect(master);
        osc.start(t);
        osc.stop(t + 0.35);
    } catch (e) {}
}
