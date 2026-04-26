<script lang="ts">
    import { listen } from "@tauri-apps/api/event";
    import { onMount } from "svelte";

    let host = "8.8.8.8";

    let currentPing = 0;
    let jitter = 0;
    let loss = 0;

    onMount(() => {
        const unlisten = listen("ping_update", (event) => {
            const data = event.payload as any;
            currentPing = data.latency;
            jitter = data.jitter;
            loss = data.loss;
        });

        return () => {
            unlisten.then((f) => f());
        };
    });
</script>

<main class="widget" data-tauri-drag-region>
    <div class="host">
        <span class="host-info">{host}</span>

        <div class="metrics">
            <div class="metric-card">
                <span class="label"
                    >Ping {currentPing > 0 ? currentPing : "--"}
                    <small>ms</small></span
                >
            </div>

            <div class="metric-card">
                <span class="label">Jitter {jitter} <small>ms</small></span>
            </div>

            <div class="metric-card">
                <span class="label">Loss {loss.toFixed(1)}<small>%</small></span
                >
            </div>
        </div>
    </div>
</main>

<style>
    [data-tauri-drag-region] {
        user-select: none;
    }

    .widget {
        display: flex;
        flex-direction: column;
        height: 100vh;
        font-family:
            system-ui,
            -apple-system,
            sans-serif;
    }

    .host {
        pointer-events: none;
    }

    .metrics {
        display: flex;
        flex-direction: row;
        justify-content: flex-start;
        gap: 7px;
    }
</style>
