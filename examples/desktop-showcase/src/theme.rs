//! The mission-control theme. One CSS const injected per window.
//!
//! Direction: the gallery's ink panel expanded into a dark operations
//! dashboard. Warm black and graphite surfaces carry the gallery's forest,
//! sage, ochre, clay, sand, and blue accents. The presentation stays quiet:
//! large charts, restrained surfaces, and no decorative status animation.
//! All fonts are local, so the showcase does not need a network.

pub const STYLE: &str = r#"
    * { box-sizing: border-box; }
    :root {
        color-scheme: dark;
        --bg: #1A1815;
        --surface: #2C2A25;
        --line: rgba(232, 229, 217, 0.12);
        --line-strong: rgba(187, 184, 174, 0.28);
        --ink: #FBFAF6;
        --ink-soft: #E8E5D9;
        --muted: #9B988D;
        --muted-soft: #7A776C;
        --forest: #3E7558;
        --forest-soft: #A6C1B0;
        --sage: #6C9984;
        --forest-pale: #F0F2E3;
        --ochre: #D5B876;
        --clay: #C9926B;
        --blue: #D9E4EC;
        --sand: #E8D4BE;
        --gold: #E9DDB8;
        --grid: rgba(232, 229, 217, 0.045);
        background: var(--bg);
    }
    body {
        margin: 0;
        height: 100vh;
        font-family: 'Poppins', ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
        color: var(--ink);
        background:
            radial-gradient(110% 80% at 30% -10%, rgba(108, 153, 132, 0.11), transparent 58%),
            repeating-linear-gradient(0deg, var(--grid) 0 1px, transparent 1px 44px),
            repeating-linear-gradient(90deg, var(--grid) 0 1px, transparent 1px 44px),
            var(--bg);
        -webkit-font-smoothing: antialiased;
        text-rendering: optimizeLegibility;
        user-select: none;
        -webkit-user-select: none;
        overflow: hidden;
    }
    .chrome { display: flex; flex-direction: column; gap: 12px; padding: 14px; height: 100vh; }

    /* ---- header ---------------------------------------------------- */
    .chrome-head {
        display: flex; align-items: center; gap: 12px;
        padding-bottom: 10px;
        border-bottom: 1px solid var(--line);
    }
    .brand {
        font-size: 15px; font-weight: 600;
        letter-spacing: 0.22em;
        color: var(--ink-soft);
    }
    .status-pill {
        margin-left: auto;
        font-family: "Cascadia Code", "JetBrains Mono", ui-monospace, monospace;
        font-size: 11px;
        color: var(--muted);
        letter-spacing: 0.04em;
        border-radius: 6px;
        padding: 4px 9px;
        background: var(--surface);
    }
    button.spawn {
        font: inherit; font-size: 12px; letter-spacing: 0.08em;
        color: var(--forest-pale);
        padding: 6px 14px;
        border: 1px solid var(--forest);
        border-radius: 8px;
        background: var(--forest);
        box-shadow: 0 1px 0 rgba(26, 24, 21, 0.04), 0 2px 6px rgba(26, 24, 21, 0.10);
        cursor: pointer;
        transition: background 160ms ease, border-color 160ms ease, transform 160ms ease;
    }
    button.spawn:hover { border-color: var(--sage); background: var(--sage); }
    button.spawn:active { transform: scale(0.98); }
    button.spawn:focus-visible, [aria-roledescription="draggable"]:focus-visible {
        outline: 2px solid var(--forest-soft);
        outline-offset: 2px;
    }

    /* ---- landing pad (drop zone) ------------------------------------ */
    .zone {
        flex: 1;
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
        gap: 14px;
        align-content: start;
        padding: 16px;
        border: 1px solid var(--line);
        border-radius: 14px;
        background: rgba(44, 42, 37, 0.58);
        overflow: auto;
        position: relative;
        transition: border-color 160ms ease, background 160ms ease;
    }
    /* Any drag in flight: every eligible pad shows marching ants. */
    .zone[data-active]::after {
        content: "";
        position: absolute; inset: 5px;
        border-radius: 10px;
        border: 1.5px dashed rgba(108, 153, 132, 0.72);
        pointer-events: none;
    }
    .zone[data-over] {
        border-color: var(--forest-soft);
        background: rgba(108, 153, 132, 0.14);
        box-shadow: inset 0 0 0 1px rgba(166, 193, 176, 0.12);
    }
    .empty {
        grid-column: 1 / -1;
        margin: auto;
        text-align: center;
        font-family: "Cascadia Code", ui-monospace, monospace;
        font-size: 12px;
        letter-spacing: 0.14em;
        color: var(--muted-soft);
    }
    .empty::before { content: "\2316  "; color: var(--forest-soft); }

    /* ---- widget cards ------------------------------------------------ */
    .slot { cursor: grab; }
    .slot:active { cursor: grabbing; }
    /* The ghost is the live one; its source dims until the drop resolves. */
    .slot[data-dragging] .widget { opacity: 0.3; filter: saturate(0.4); }
    .widget {
        --accent: var(--forest-soft);
        position: relative;
        border: 1px solid var(--line);
        border-radius: 10px;
        background: var(--surface);
        padding: 15px 16px;
        min-height: 146px;
        height: 100%;
        display: flex; flex-direction: column; gap: 11px;
        box-shadow: 0 1px 0 rgba(255, 255, 255, 0.025),
                    0 8px 20px -16px rgba(0, 0, 0, 0.85);
        overflow: hidden;
    }
    .widget[data-kind="sparkline"] { --accent: var(--forest-soft); }
    .widget[data-kind="stopwatch"] { --accent: var(--ochre); }
    .widget[data-kind="ring"]      { --accent: var(--sage); }
    .widget[data-kind="pulse"]     { --accent: var(--clay); }
    .widget[data-kind="bars"]      { --accent: var(--blue); }
    .widget[data-kind="area"]      { --accent: var(--sand); }
    .widget[data-kind="radar"]     { --accent: var(--gold); }
    .widget-head {
        display: flex; align-items: center;
        font-size: 11px; font-weight: 600;
        letter-spacing: 0.16em; text-transform: uppercase;
        color: var(--accent);
    }
    .widget-body {
        flex: 1;
        display: flex; flex-direction: column; justify-content: space-between; gap: 8px;
        color: var(--accent);
    }
    .spark, .ecg, .bars, .area, .radar { width: 100%; height: 72px; }
    .ring { width: 72px; height: 72px; }
    .ring-track, .chart-grid { stroke: var(--line-strong); stroke-width: 0.8; }
    .chart-bar { fill: currentColor; opacity: 0.82; }
    .area-fill, .radar-fill { fill: currentColor; opacity: 0.16; }
    .area-line, .radar-line { fill: none; stroke: currentColor; stroke-width: 1.6; }
    .radar-spoke { stroke: var(--line); stroke-width: 0.8; }
    .clock {
        font-family: "Cascadia Code", "JetBrains Mono", ui-monospace, monospace;
        font-size: 34px; font-weight: 350;
        font-variant-numeric: tabular-nums;
    }
    .widget[data-kind="stopwatch"] .widget-body { justify-content: center; gap: 10px; }
    .readout {
        font-family: "Cascadia Code", ui-monospace, monospace;
        font-size: 10.5px;
        letter-spacing: 0.08em;
        color: var(--muted);
    }

    /* ---- the ghost ----------------------------------------------------
       Rendered by whichever window presents the drag this frame; the card
       inside keeps streaming because it reads the live payload signal. */
    .ghost .widget {
        border-color: var(--accent);
        box-shadow:
            0 0 0 1px color-mix(in srgb, var(--accent) 36%, transparent),
            0 20px 44px rgba(0, 0, 0, 0.52);
        transform: scale(1.015);
    }
"#;
