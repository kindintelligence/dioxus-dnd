//! The mission-control theme. One CSS const injected per window.
//!
//! Direction: NASA ops console with a CRT tinge - deep space-blue base, a
//! faint HUD grid, DIN-style brand type (Bahnschrift ships with Windows 11;
//! elsewhere it falls back), phosphor monospace readouts (Cascadia Code),
//! one neon accent per widget kind carried by `--accent` on `data-kind`,
//! marching-ants landing pads while a drag is in flight, and a lifted,
//! glowing ghost. All fonts are local; the showcase must not need a network.

pub const STYLE: &str = r#"
    * { box-sizing: border-box; }
    :root {
        --bg: #070a12;
        --panel: rgba(16, 22, 36, 0.82);
        --panel-edge: rgba(140, 180, 255, 0.16);
        --ink: #d9e4ef;
        --ink-dim: rgba(217, 228, 239, 0.55);
        --grid: rgba(140, 180, 255, 0.05);
        --teal: #3ddbd9;
    }
    body {
        margin: 0;
        height: 100vh;
        font-family: Bahnschrift, "Segoe UI", "DejaVu Sans", sans-serif;
        color: var(--ink);
        background:
            radial-gradient(120% 90% at 30% -10%, rgba(61, 219, 217, 0.07), transparent 60%),
            repeating-linear-gradient(0deg, var(--grid) 0 1px, transparent 1px 44px),
            repeating-linear-gradient(90deg, var(--grid) 0 1px, transparent 1px 44px),
            var(--bg);
        user-select: none;
        -webkit-user-select: none;
        overflow: hidden;
    }
    .chrome { display: flex; flex-direction: column; gap: 12px; padding: 14px; height: 100vh; }

    /* ---- header ---------------------------------------------------- */
    .chrome-head {
        display: flex; align-items: center; gap: 12px;
        padding-bottom: 10px;
        border-bottom: 1px solid var(--panel-edge);
    }
    .brand {
        font-size: 15px; font-weight: 600;
        letter-spacing: 0.22em;
    }
    .brand::before {
        content: "";
        display: inline-block;
        width: 8px; height: 8px; border-radius: 50%;
        margin-right: 10px;
        background: var(--teal);
        box-shadow: 0 0 8px var(--teal);
        animation: led 2.4s ease-in-out infinite;
        vertical-align: 1px;
    }
    @keyframes led { 50% { opacity: 0.35; box-shadow: 0 0 2px var(--teal); } }
    .status-pill {
        margin-left: auto;
        font-family: "Cascadia Code", "JetBrains Mono", ui-monospace, monospace;
        font-size: 11px;
        color: var(--ink-dim);
        border: 1px solid var(--panel-edge);
        border-radius: 999px;
        padding: 3px 10px;
        background: rgba(10, 14, 24, 0.6);
    }
    button.spawn {
        font: inherit; font-size: 12px; letter-spacing: 0.08em;
        color: var(--ink);
        padding: 6px 14px;
        border: 1px solid var(--panel-edge);
        border-radius: 8px;
        background: linear-gradient(180deg, rgba(61, 219, 217, 0.16), rgba(61, 219, 217, 0.05));
        cursor: pointer;
    }
    button.spawn:hover { border-color: var(--teal); box-shadow: 0 0 12px rgba(61, 219, 217, 0.35); }

    /* ---- landing pad (drop zone) ------------------------------------ */
    .zone {
        flex: 1;
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
        gap: 12px;
        align-content: start;
        padding: 14px;
        border: 1px solid var(--panel-edge);
        border-radius: 14px;
        background: rgba(9, 13, 22, 0.55);
        overflow: auto;
        position: relative;
        transition: box-shadow 160ms ease, border-color 160ms ease;
    }
    /* Any drag in flight: every eligible pad shows marching ants. */
    .zone[data-active]::after {
        content: "";
        position: absolute; inset: 5px;
        border-radius: 10px;
        border: 1.5px dashed rgba(61, 219, 217, 0.45);
        pointer-events: none;
        animation: ants 1.1s linear infinite;
        -webkit-mask: conic-gradient(#000 0 0);
    }
    @keyframes ants { to { transform: rotate(0.0001deg); opacity: 0.55; } }
    .zone[data-over] {
        border-color: var(--teal);
        box-shadow: 0 0 0 1px rgba(61, 219, 217, 0.5), 0 0 34px rgba(61, 219, 217, 0.22),
                    inset 0 0 40px rgba(61, 219, 217, 0.07);
    }
    .empty {
        grid-column: 1 / -1;
        margin: auto;
        text-align: center;
        font-family: "Cascadia Code", ui-monospace, monospace;
        font-size: 12px;
        letter-spacing: 0.14em;
        color: var(--ink-dim);
        animation: led 3s ease-in-out infinite;
    }
    .empty::before { content: "\2316  "; color: var(--teal); }

    /* ---- widget cards ------------------------------------------------ */
    .slot { cursor: grab; }
    .slot:active { cursor: grabbing; }
    .widget {
        --accent: var(--teal);
        position: relative;
        border: 1px solid var(--panel-edge);
        border-radius: 12px;
        background: linear-gradient(180deg, rgba(24, 32, 50, 0.85), var(--panel));
        backdrop-filter: blur(6px);
        padding: 12px 14px;
        display: flex; flex-direction: column; gap: 8px;
        box-shadow: 0 4px 18px rgba(0, 0, 0, 0.35);
        overflow: hidden;
    }
    .widget::before {
        content: "";
        position: absolute; inset: 0 0 auto 0; height: 2px;
        background: linear-gradient(90deg, transparent, var(--accent), transparent);
        opacity: 0.8;
    }
    .widget[data-kind="sparkline"] { --accent: #3ddbd9; }
    .widget[data-kind="stopwatch"] { --accent: #ffb454; }
    .widget[data-kind="ring"]      { --accent: #3fd97b; }
    .widget[data-kind="pulse"]     { --accent: #ff5d6c; }
    .widget-head {
        display: flex; align-items: center; gap: 8px;
        font-size: 10.5px; font-weight: 600;
        letter-spacing: 0.18em; text-transform: uppercase;
        color: var(--ink-dim);
    }
    .widget-dot {
        width: 7px; height: 7px; border-radius: 50%;
        background: var(--accent);
        box-shadow: 0 0 8px var(--accent);
        animation: led 1.8s ease-in-out infinite;
    }
    .widget-body {
        display: flex; flex-direction: column; gap: 5px;
        color: var(--accent);
    }
    .spark, .ecg { width: 100%; height: 46px; filter: drop-shadow(0 0 5px var(--accent)); }
    .ring { width: 48px; height: 48px; filter: drop-shadow(0 0 5px var(--accent)); }
    .ring-track { stroke: rgba(140, 180, 255, 0.14); }
    .clock {
        font-family: "Cascadia Code", "JetBrains Mono", ui-monospace, monospace;
        font-size: 27px; font-weight: 350;
        text-shadow: 0 0 14px var(--accent);
        font-variant-numeric: tabular-nums;
    }
    .readout {
        font-family: "Cascadia Code", ui-monospace, monospace;
        font-size: 11px;
        letter-spacing: 0.1em;
        color: var(--ink-dim);
    }

    /* ---- the ghost ----------------------------------------------------
       Rendered by whichever window presents the drag this frame; the card
       inside keeps streaming because it reads the live payload signal. */
    .ghost .widget {
        border-color: var(--accent);
        box-shadow:
            0 0 0 1px var(--accent),
            0 0 30px color-mix(in srgb, var(--accent) 45%, transparent),
            0 22px 50px rgba(0, 0, 0, 0.65);
        transform: scale(1.03);
    }
"#;
