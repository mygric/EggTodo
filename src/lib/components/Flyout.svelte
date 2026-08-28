<script lang="ts">
  import { onMount } from "svelte";
  import { isTauri } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
  import { todoApi } from "$lib/api/todoApi";

  const POSITION_STORAGE_KEY = "eggdone-flyout-position-v2";
  const REFRESH_INTERVAL_MS = 30_000;
  const CLICK_THRESHOLD_PX = 8;
  const CLICK_MAX_DURATION_MS = 500;

  let count = 0;

  // --- Click vs drag detection ---
  let pointerDownScreenX = 0;
  let pointerDownScreenY = 0;
  let pointerDownTime = 0;
  let pointerActive = false;

  // --- JS-driven window dragging ---
  // We do NOT use startDragging() because it launches a system-level drag
  // that causes micro-movements to be registered as drags, making every
  // click look like a drag. Instead we move the window ourselves via
  // setPosition, which keeps pointer events fully under our control.
  let dragActive = false;
  let windowDragged = false;
  let dragMoveStarted = false;
  let dragWindowPosReady = false;
  let dragWindowStartX = 0;
  let dragWindowStartY = 0;
  let dragPointerStartX = 0;
  let dragPointerStartY = 0;

  async function refreshCount() {
    if (!isTauri()) return;
    try {
      count = await todoApi.countTodayDue();
    } catch (error) {
      console.error("[flyout] refreshCount failed:", error);
    }
  }

  function refreshCountOnFocus() {
    void refreshCount();
  }

  async function handlePointerDown(event: PointerEvent) {
    if (event.button !== 0) return;

    // --- Synchronous bookkeeping (before any await) ---
    pointerDownScreenX = event.screenX;
    pointerDownScreenY = event.screenY;
    pointerDownTime = Date.now();
    pointerActive = true;
    dragActive = true;
    windowDragged = false;
    dragMoveStarted = false;
    dragWindowPosReady = false;

    // CRITICAL: capture pointer synchronously so pointermove keeps firing
    // even when the cursor leaves the element during a drag.
    if (event.currentTarget instanceof Element) {
      event.currentTarget.setPointerCapture(event.pointerId);
    }

    // Suppress the main panel's blur-to-hide so that flyoutTogglePanel()
    // sees the true visibility state at pointer-up. Without this, main
    // auto-hides on blur before we can toggle, making every second click
    // re-show instead of hide.
    if (isTauri()) {
      void todoApi.markPanelInteraction().catch(() => {});
    }

    // --- Async work ---

    // Fetch the window's starting position for JS-driven dragging.
    if (isTauri()) {
      try {
        const pos = await getCurrentWindow().outerPosition();
        dragWindowStartX = pos.x;
        dragWindowStartY = pos.y;
        dragWindowPosReady = true;
      } catch (e) {
        console.error("[flyout] outerPosition failed:", e);
      }
    }
  }

  function handlePointerMove(event: PointerEvent) {
    if (!pointerActive || !dragActive) return;

    const dx = event.screenX - pointerDownScreenX;
    const dy = event.screenY - pointerDownScreenY;

    // Once past the click threshold, this is a drag (suppress toggle on up).
    if (!windowDragged && Math.hypot(dx, dy) > CLICK_THRESHOLD_PX) {
      windowDragged = true;
    }

    if (!windowDragged || !dragWindowPosReady || !isTauri()) return;

    // Anchor pointer reference on first valid move to avoid a jump from the
    // async outerPosition() call.
    if (!dragMoveStarted) {
      dragPointerStartX = event.screenX;
      dragPointerStartY = event.screenY;
      dragMoveStarted = true;
    }

    const newX = dragWindowStartX + (event.screenX - dragPointerStartX);
    const newY = dragWindowStartY + (event.screenY - dragPointerStartY);
    void getCurrentWindow()
      .setPosition(new PhysicalPosition(newX, newY))
      .catch(() => {
        // ignore transient position-set failures
      });
  }

  function handlePointerUp(event: PointerEvent) {
    if (!pointerActive) return;
    pointerActive = false;
    dragActive = false;

    if (windowDragged) return; // was a drag, not a click

    const duration = Date.now() - pointerDownTime;
    if (duration >= CLICK_MAX_DURATION_MS) return; // long press

    // markPanelInteraction() at pointer-down suppressed the blur-to-hide,
    // so the main panel's visibility is still in its pre-click state.
    // flyoutTogglePanel() queries is_visible() on the Rust side and toggles
    // reliably — no frontend state to desync.
    void todoApi
      .flyoutTogglePanel()
      .catch((e) => console.error("[flyout] toggle failed:", e));
  }

  async function restorePosition() {
    if (!isTauri()) return;
    const saved = localStorage.getItem(POSITION_STORAGE_KEY);
    if (!saved) return;
    try {
      const { x, y } = JSON.parse(saved) as { x: number; y: number };
      if (typeof x === "number" && typeof y === "number") {
        await getCurrentWindow().setPosition(new PhysicalPosition(x, y));
      }
    } catch (error) {
      console.error("[flyout] restorePosition failed:", error);
    }
  }

  onMount(() => {
    const unlisteners: UnlistenFn[] = [];

    // Respect the user's floating-ball preference from settings.
    if (isTauri() && localStorage.getItem("eggdone-floating-ball-enabled") === "false") {
      void getCurrentWindow().hide().catch(() => {});
    }

    void restorePosition();
    void refreshCount();

    if (isTauri()) {
      void listen("todos-changed", () => void refreshCount()).then((unlisten) => {
        unlisteners.push(unlisten);
      });
      window.addEventListener("focus", refreshCountOnFocus);
      document.addEventListener("visibilitychange", refreshCountOnFocus);
      void getCurrentWindow()
        .onMoved(({ payload }) => {
          try {
            localStorage.setItem(
              POSITION_STORAGE_KEY,
              JSON.stringify({ x: payload.x, y: payload.y }),
            );
          } catch {
            // ignore
          }
        })
        .then((unlisten) => unlisteners.push(unlisten));
    }
    const interval = window.setInterval(
      () => void refreshCount(),
      REFRESH_INTERVAL_MS,
    );

    return () => {
      unlisteners.forEach((unlisten) => unlisten());
      window.removeEventListener("focus", refreshCountOnFocus);
      document.removeEventListener("visibilitychange", refreshCountOnFocus);
      window.clearInterval(interval);
    };
  });
</script>

<div
  class="flyout"
  role="button"
  tabindex="0"
  aria-label="蛋定 Todo"
  title="蛋定 Todo"
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
>
  <img src="/eggdone-icon.png" alt="EggDone" class="flyout-icon" />
  {#if count > 0}
    <span class="count">{count}</span>
  {/if}
</div>

<style>
  /* Reset default body margin so the 70x70 window has no dead space. */
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: transparent;
  }

  .flyout {
    position: fixed;
    top: 0;
    left: 0;
    width: 70px;
    height: 70px;
    border-radius: 50%;
    cursor: grab;
    user-select: none;
    background: transparent;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    touch-action: none;
    transition: transform 0.15s ease;
  }
  .flyout:hover {
    transform: scale(1.08);
  }
  .flyout:active {
    cursor: grabbing;
  }
  .flyout-icon {
    width: 64px;
    height: 64px;
    object-fit: contain;
    pointer-events: none;
  }
  .count {
    position: absolute;
    right: 6px;
    bottom: 18px;
    background: #f6c94c;
    border: 1px solid #000;
    border-radius: 7px;
    min-width: 18px;
    height: 18px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 700;
    color: #3d3528;
    padding: 0 4px;
    pointer-events: none;
  }
</style>
