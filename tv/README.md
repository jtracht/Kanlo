# Native TV adapter — reserved module boundary

This directory reserves the TV part of the monorepo. There is no renderer,
executable, overlay/rotation controller or TV service here yet. KAN-027–034
remain open until implementation and physical Pi/display acceptance.

The future native process reads the same server authority as the browser and
consumes its versioned event/revision stream. It must keep display/view/overlay
state independent of browser state, use the shared domain mutation rules and
never make a browser kiosk a mandatory substitute for native graphics.

Choose graphics/font/image libraries after the real Pi 3 B+ ARM32 prototype;
measure memory, latency and frame behavior before assigning resource budgets.
The HTTP server must continue starting and operating when no display is present.
