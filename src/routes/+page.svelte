<script lang="ts">
    import { listen } from "@tauri-apps/api/event";
    import { invoke } from "@tauri-apps/api/core";
    import { onMount, tick } from "svelte";

    interface HostEntry {
        label: string;
        address: string;
        port: number;
    }

    interface ColumnConfig {
        latency: boolean;
        jitter: boolean;
        loss: boolean;
    }

    interface Settings {
        hosts: HostEntry[];
        columns: ColumnConfig;
    }

    interface HostMetrics {
        latency: number;
        jitter: number;
        loss: number;
    }

    let hosts: HostEntry[] = [];
    let columns: ColumnConfig = { latency: true, jitter: true, loss: true };
    let metrics: Record<string, HostMetrics> = {};
    let widgetEl: HTMLElement;

    async function updateSize() {
        await tick();
        if (!widgetEl) return;
        const rect = widgetEl.getBoundingClientRect();
        await invoke("resize_widget", {
            width: Math.ceil(rect.width),
            height: Math.ceil(rect.height),
        });
    }

    let dragTimer: ReturnType<typeof setTimeout> | null = null;
    function onDragMove() {
        if (dragTimer) clearTimeout(dragTimer);
        dragTimer = setTimeout(
            () => invoke("save_position").catch(() => {}),
            400,
        );
    }

    onMount(() => {
        document.addEventListener("contextmenu", (e) => e.preventDefault());

        let unlistenPing: (() => void) | null = null;
        let unlistenSettings: (() => void) | null = null;

        (async () => {
            const s = await invoke<Settings>("get_settings");
            hosts = s.hosts;
            columns = s.columns;
            await updateSize();

            unlistenPing = await listen("ping_update", (event) => {
                const d = event.payload as any;
                metrics[d.host] = {
                    latency: d.latency,
                    jitter: d.jitter,
                    loss: d.loss,
                };
                metrics = metrics;
            });

            unlistenSettings = await listen<Settings>(
                "settings_updated",
                async (event) => {
                    hosts = event.payload.hosts;
                    columns = event.payload.columns;
                    metrics = {};
                    await updateSize();
                },
            );
        })();

        return () => {
            unlistenPing?.();
            unlistenSettings?.();
        };
    });

    function status(
        m: HostMetrics | undefined,
    ): "good" | "medium" | "bad" | "unknown" {
        if (!m || m.latency === 0) return m?.loss === 100 ? "bad" : "unknown";
        if (m.latency < 80) return "good";
        if (m.latency < 200) return "medium";
        return "bad";
    }

    // U+2007 FIGURE SPACE has the same advance width as a digit in tabular fonts.
    // We pad both numbers and the placeholder to the same 3-character width so
    // the layout never shifts regardless of the value shown.
    const FIGURE_SPACE = "\u2007";
    const PLACEHOLDER = FIGURE_SPACE + "--";
    function fmt(n: number | undefined): string {
        if (n === undefined || n === 0) return PLACEHOLDER;
        return String(n).padStart(3, FIGURE_SPACE);
    }

    function fmtLoss(m: HostMetrics | undefined): string {
        if (!m) return PLACEHOLDER;
        return m.loss.toFixed(0).padStart(3, FIGURE_SPACE);
    }
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<main
    bind:this={widgetEl}
    class="widget"
    data-tauri-drag-region
    on:mousemove={onDragMove}
>
    {#each hosts as host}
        {@const m = metrics[host.label]}
        {@const s = status(m)}
        <div class="row">
            <span class="dot dot-{s}"></span>
            <span class="lbl">{host.label}</span>
            <div class="nums">
                {#if columns.latency}
                    <span class="num">{fmt(m?.latency)}<sub>ms</sub></span>
                {/if}
                {#if columns.jitter}
                    {#if columns.latency}<span class="sep">·</span>{/if}
                    <span class="num">{fmt(m?.jitter)}<sub>jit</sub></span>
                {/if}
                {#if columns.loss}
                    {#if columns.latency || columns.jitter}<span class="sep"
                            >·</span
                        >{/if}
                    <span class="num {m && m.loss > 0 ? 'loss-warn' : ''}"
                        >{fmtLoss(m)}<sub>%</sub></span
                    >
                {/if}
            </div>
        </div>
    {/each}
</main>

<style>
    :global(html, body) {
        margin: 0;
        padding: 0;
        overflow: hidden;
        background: transparent;
        display: flex;
        width: fit-content;
        height: fit-content;
    }

    .widget {
        display: flex;
        flex-direction: column;
        gap: 5px;
        padding: 9px 13px;

        backdrop-filter: blur(40px) saturate(1.8) brightness(0.5);
        -webkit-backdrop-filter: blur(40px) saturate(1.8) brightness(0.5);

        background: rgba(255, 255, 255, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.18);
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.25);
        border-radius: 13px;

        font-family:
            system-ui,
            -apple-system,
            sans-serif;
        color: rgba(255, 255, 255, 0.8);
        user-select: none;
        white-space: nowrap;
    }

    .row {
        display: flex;
        align-items: center;
        gap: 7px;
        pointer-events: none;
        margin-right: 4px;
    }

    .dot {
        flex-shrink: 0;
        width: 7px;
        height: 7px;
        border-radius: 50%;
    }
    .dot-good {
        background: #4ade80;
        box-shadow: 0 0 6px #4ade80cc;
    }
    .dot-medium {
        background: #facc15;
        box-shadow: 0 0 6px #facc15cc;
    }
    .dot-bad {
        background: #f87171;
        box-shadow: 0 0 6px #f87171cc;
    }
    .dot-unknown {
        background: rgba(255, 255, 255, 0.25);
    }

    .lbl {
        font-size: 12px;
        font-weight: 500;
        color: rgba(255, 255, 255, 0.92);
        min-width: 54px;
        text-shadow: 0 1px 4px rgba(0, 0, 0, 0.6);
    }

    .nums {
        display: flex;
        align-items: baseline;
        gap: 4px;
    }

    .num {
        display: inline-block;
        /*
         * Width is fixed at exactly 3 tabular digits + sub label.
         * We do NOT use min-width here because different fonts render
         * figure-space slightly differently — instead the JS padding
         * guarantees a stable 3-char string at all times.
         */
        width: 4ch;
        text-align: right;
        font-size: 13px;
        font-weight: 600;
        font-variant-numeric: tabular-nums;
        font-feature-settings: "tnum";
        color: rgba(255, 255, 255, 0.8);
        text-shadow: 0 1px 4px rgba(0, 0, 0, 0.6);
    }

    .num sub {
        font-size: 9px;
        font-weight: 400;
        opacity: 0.8;
        margin-left: 1px;
        vertical-align: baseline;
        position: relative;
    }

    .loss-warn {
        color: #fb923c;
    }

    .sep {
        color: rgba(255, 255, 255, 0.2);
        font-size: 10px;
    }
</style>
