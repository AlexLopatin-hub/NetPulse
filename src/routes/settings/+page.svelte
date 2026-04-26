<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { onMount } from "svelte";

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

    let hosts: HostEntry[] = [];
    let columns: ColumnConfig = { latency: true, jitter: false, loss: false };
    let saving = false;
    let error = "";

    onMount(async () => {
        const s = await invoke<Settings>("get_settings");
        hosts = s.hosts;
        columns = s.columns;
    });

    function addHost() {
        hosts = [...hosts, { label: "", address: "", port: 80 }];
    }

    function removeHost(i: number) {
        hosts = hosts.filter((_, idx) => idx !== i);
    }

    async function save() {
        error = "";
        const valid = hosts.filter((h) => h.label.trim() && h.address.trim());
        if (valid.length === 0) {
            error = "Добавьте хотя бы один хост.";
            return;
        }
        if (!columns.latency && !columns.jitter && !columns.loss) {
            error = "Выберите хотя бы один показатель для отображения.";
            return;
        }
        saving = true;
        try {
            await invoke("save_settings", {
                settings: { hosts: valid, columns },
            });
        } catch (e) {
            error = String(e);
        } finally {
            saving = false;
        }
    }

    function cancel() {
        getCurrentWindow().close();
    }
</script>

<div class="page">
    <section>
        <h3 class="section-title">Хосты</h3>

        <div class="table-head">
            <span>Название</span>
            <span>Адрес</span>
            <span>Порт</span>
            <span></span>
        </div>

        <div class="list">
            {#each hosts as host, i}
                <div class="host-row">
                    <input
                        class="inp"
                        bind:value={host.label}
                        placeholder="YouTube"
                        maxlength="20"
                    />
                    <input
                        class="inp"
                        bind:value={host.address}
                        placeholder="youtube.com"
                    />
                    <input
                        class="inp inp-port"
                        type="number"
                        bind:value={host.port}
                        min="1"
                        max="65535"
                    />
                    <button
                        class="rm-btn"
                        on:click={() => removeHost(i)}
                        title="Удалить">✕</button
                    >
                </div>
            {/each}

            <button class="add-btn" on:click={addHost}>+ Добавить хост</button>
        </div>
    </section>

    <section>
        <h3 class="section-title">Показатели</h3>
        <div class="checks">
            <label class="check-label">
                <input type="checkbox" bind:checked={columns.latency} />
                <span class="check-box"></span>
                Задержка <span class="hint">ms</span>
            </label>
            <label class="check-label">
                <input type="checkbox" bind:checked={columns.jitter} />
                <span class="check-box"></span>
                Джиттер <span class="hint">jit</span>
            </label>
            <label class="check-label">
                <input type="checkbox" bind:checked={columns.loss} />
                <span class="check-box"></span>
                Потери <span class="hint">%</span>
            </label>
        </div>
    </section>

    {#if error}
        <p class="error">{error}</p>
    {/if}

    <div class="footer">
        <button class="btn-cancel" on:click={cancel}>Отмена</button>
        <button class="btn-save" on:click={save} disabled={saving}>
            {saving ? "Сохранение…" : "Сохранить"}
        </button>
    </div>
</div>

<style>
    :global(body) {
        margin: 0;
        font-family:
            system-ui,
            -apple-system,
            sans-serif;
        font-size: 13px;
        color: #111;
        background: #f4f4f6;
    }

    .page {
        display: flex;
        flex-direction: column;
        gap: 16px;
        padding: 18px 20px 16px;
        height: 100vh;
        box-sizing: border-box;
    }

    section {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }

    .section-title {
        margin: 0;
        font-size: 11px;
        font-weight: 600;
        letter-spacing: 0.06em;
        text-transform: uppercase;
        color: #888;
    }

    .table-head {
        display: grid;
        grid-template-columns: 110px 1fr 64px 24px;
        gap: 6px;
        font-size: 11px;
        color: #aaa;
        padding: 0 2px;
    }

    .list {
        display: flex;
        flex-direction: column;
        gap: 5px;
        overflow-y: auto;
        max-height: 200px;
    }

    .host-row {
        display: grid;
        grid-template-columns: 110px 1fr 64px 24px;
        gap: 6px;
        align-items: center;
    }

    .inp {
        padding: 5px 8px;
        border: 1px solid #d8d8df;
        border-radius: 7px;
        font-size: 13px;
        background: #fff;
        color: #111;
        outline: none;
        width: 100%;
        box-sizing: border-box;
        transition:
            border-color 0.12s,
            box-shadow 0.12s;
    }
    .inp:focus {
        border-color: #007aff;
        box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
    }
    .inp-port {
        font-variant-numeric: tabular-nums;
    }

    .rm-btn {
        background: none;
        border: none;
        color: #e05555;
        font-size: 13px;
        cursor: pointer;
        padding: 0;
        opacity: 0.65;
        transition: opacity 0.12s;
        text-align: center;
    }
    .rm-btn:hover {
        opacity: 1;
    }

    .add-btn {
        align-self: flex-start;
        background: none;
        border: 1px dashed #c0c0cc;
        border-radius: 7px;
        color: #666;
        font-size: 12px;
        padding: 5px 12px;
        cursor: pointer;
        transition:
            border-color 0.12s,
            color 0.12s;
    }
    .add-btn:hover {
        border-color: #007aff;
        color: #007aff;
    }

    .checks {
        display: flex;
        gap: 6px;
    }

    .check-label {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 7px 12px;
        border: 1px solid #d8d8df;
        border-radius: 8px;
        background: #fff;
        cursor: pointer;
        font-size: 13px;
        transition:
            border-color 0.12s,
            background 0.12s;
        user-select: none;
    }
    .check-label:hover {
        border-color: #007aff;
    }

    .check-label input[type="checkbox"] {
        position: absolute;
        opacity: 0;
        width: 0;
        height: 0;
    }

    .check-box {
        width: 15px;
        height: 15px;
        border-radius: 4px;
        border: 1.5px solid #c0c0cc;
        background: #fff;
        flex-shrink: 0;
        position: relative;
        transition:
            background 0.12s,
            border-color 0.12s;
    }

    .check-label input[type="checkbox"]:checked + .check-box {
        background: #007aff;
        border-color: #007aff;
    }
    .check-label input[type="checkbox"]:checked + .check-box::after {
        content: "";
        position: absolute;
        left: 3px;
        top: 1px;
        width: 5px;
        height: 8px;
        border: 2px solid #fff;
        border-top: none;
        border-left: none;
        transform: rotate(45deg);
    }

    .check-label input[type="checkbox"]:checked ~ * {
    }

    .hint {
        font-size: 10px;
        color: #aaa;
        font-variant-numeric: tabular-nums;
    }

    .error {
        margin: 0;
        color: #cc2222;
        font-size: 12px;
    }

    .footer {
        display: flex;
        justify-content: flex-end;
        gap: 8px;
        padding-top: 12px;
        border-top: 1px solid #e0e0e8;
        margin-top: auto;
    }

    .btn-cancel,
    .btn-save {
        padding: 7px 18px;
        border-radius: 8px;
        font-size: 13px;
        font-weight: 500;
        cursor: pointer;
        border: none;
        transition: opacity 0.12s;
    }
    .btn-cancel {
        background: #e4e4ec;
        color: #333;
    }
    .btn-save {
        background: #007aff;
        color: #fff;
    }
    .btn-save:disabled {
        opacity: 0.55;
        cursor: default;
    }
    .btn-cancel:hover {
        opacity: 0.8;
    }
    .btn-save:not(:disabled):hover {
        opacity: 0.85;
    }

    @media (prefers-color-scheme: dark) {
        :global(body) {
            background: #1c1c1e;
            color: #f0f0f5;
        }
        .section-title {
            color: #666;
        }
        .inp {
            background: #2a2a2e;
            border-color: #3a3a42;
            color: #f0f0f5;
        }
        .inp:focus {
            border-color: #0a84ff;
            box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.18);
        }
        .add-btn {
            border-color: #3a3a42;
            color: #999;
        }
        .add-btn:hover {
            border-color: #0a84ff;
            color: #0a84ff;
        }
        .check-label {
            background: #2a2a2e;
            border-color: #3a3a42;
            color: #e8e8f0;
        }
        .check-label:hover {
            border-color: #0a84ff;
        }
        .check-box {
            background: #1c1c1e;
            border-color: #3a3a42;
        }
        .check-label input[type="checkbox"]:checked + .check-box {
            background: #0a84ff;
            border-color: #0a84ff;
        }
        .table-head {
            color: #555;
        }
        .rm-btn {
            color: #f87171;
        }
        .btn-cancel {
            background: #3a3a42;
            color: #ddd;
        }
        .btn-save {
            background: #0a84ff;
        }
        .footer {
            border-top-color: #3a3a42;
        }
        .hint {
            color: #666;
        }
    }
</style>
