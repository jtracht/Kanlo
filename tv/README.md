# Native Kanlo TV display

The appliance runs an SDL/Wayland viewer in Cage. It subscribes to the server's
event stream and retains a simple console board when graphics cannot start.
See [appliance setup](../release/IMAGES-EN.md) for display and Wi-Fi setup.

The web app's **TV display** dialog selects a board, the global overview or
statistics. Rotation is off by default. Select the rotation views and their
order, then set the interval. Opening a web ticket does not affect the screen.
Use **Show on TV** to open details or **Highlight on TV** to focus its board card.
The dialog also opens the backlog, scrolls the display, and returns to earlier
overlay contexts. TV controls require a live connection.

`GET /api/v1/display` returns the local display state and `POST` applies an
explicit display action. The same API serves the web app and both MCP plugins.
Display commands do not change tickets or the browser's independent view.
The current selection and overlay context survive a server restart. Restore
clears old overlay and highlight context.

The current native detail renderer displays descriptions, comments and linked
ticket identifiers. Rich Markdown formatting and remote images remain open.
Physical Pi/HDMI/composite/Wi-Fi and long-running display measurements remain
separate acceptance work.
