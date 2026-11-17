"""Present the software-rendered Pygame board through GTK's Wayland surface."""

RGB24_MASKS = (0x00FF0000, 0x0000FF00, 0x000000FF, 0)


class WaylandDisplay:
    def __init__(self, pygame, size, on_frame):
        import gi
        gi.require_version('Gtk', '3.0')
        gi.require_version('Gdk', '3.0')
        import cairo
        from gi.repository import Gdk, GLib, Gtk

        self.pygame = pygame
        self.gtk = Gtk
        self.gdk = Gdk
        self.glib = GLib
        self.loop = GLib.MainLoop()
        self.cairo = cairo
        self.on_frame = on_frame
        self.frame = None
        self.tick_id = None
        self.accent_width = 0
        self.surface = pygame.Surface(size, depth=32, masks=RGB24_MASKS)
        self.window = Gtk.Window(title='Kanlo')
        self.window.set_default_size(*size)
        self.window.set_decorated(False)
        self.window.connect('delete-event', self.quit)
        self.window.connect('key-press-event', self.key)
        self.window.connect('realize', self.hide_cursor)
        self.area = Gtk.DrawingArea()
        self.area.connect('size-allocate', self.resize)
        self.area.connect('draw', self.draw)
        self.window.add(self.area)
        self.window.fullscreen()
        self.window.show_all()

    def hide_cursor(self, window):
        cursor = self.gdk.Cursor.new_from_name(window.get_display(), 'none')
        window.get_window().set_cursor(cursor)

    def quit(self, *_):
        self.pygame.event.post(self.pygame.event.Event(self.pygame.QUIT))
        self.loop.quit()
        return True

    def key(self, _, event):
        keys = {self.gdk.KEY_Left: self.pygame.K_LEFT, self.gdk.KEY_Right: self.pygame.K_RIGHT,
                self.gdk.KEY_space: self.pygame.K_SPACE, self.gdk.KEY_Escape: self.pygame.K_ESCAPE}
        if event.keyval in keys:
            self.pygame.event.post(self.pygame.event.Event(self.pygame.KEYDOWN, key=keys[event.keyval]))
            self.loop.quit()
        return False

    def resize(self, _, allocation):
        size = allocation.width, allocation.height
        if size != self.surface.get_size() and min(size) > 1:
            self.stop_animation()
            self.frame = None
            self.surface = self.pygame.Surface(size, depth=32, masks=RGB24_MASKS)

    def draw(self, _, context):
        if self.frame is None:
            return False
        context.set_source_surface(self.frame)
        context.paint()
        if self.accent_width:
            context.set_source_rgb(0.19, 0.35, 0.74)
            context.rectangle(0, 0, self.accent_width, 3)
            context.fill()
        self.on_frame()
        return False

    def stop_animation(self):
        if self.tick_id is not None:
            self.area.remove_tick_callback(self.tick_id)
            self.tick_id = None
        self.accent_width = 0

    def present(self, duration=0, region=None):
        width, height = self.surface.get_size()
        if region is not None and self.frame is not None:
            # The footer is opaque. Update its rows without copying the board
            # or interrupting an animation already running in the header.
            self.frame.flush()
            pitch = self.surface.get_pitch()
            start, end = region.top * pitch, region.bottom * pitch
            self.frame.get_data()[start:end] = memoryview(self.surface.get_view('0'))[start:end]
            self.frame.mark_dirty_rectangle(0, region.top, width, region.height)
            self.area.queue_draw_area(0, region.top, width, region.height)
            return
        self.stop_animation()
        # Both surfaces use native-endian 0x00RRGGBB. Copy once so Pygame can
        # draw the next scene while GTK still presents this one.
        pixels = bytearray(self.surface.get_view('0'))
        has_frame = self.frame is not None
        self.frame = self.cairo.ImageSurface.create_for_data(
            pixels, self.cairo.FORMAT_RGB24, width, height, self.surface.get_pitch(),
        )
        if duration and has_frame and self.gtk.Settings.get_default().get_property('gtk-enable-animations'):
            # Keep damage to a thin accent. Fullscreen slides force both Cairo
            # and a software compositor to move millions of pixels per tick.
            extent = min(width // 4, 240)
            start = None

            def advance(widget, clock):
                nonlocal start
                now = clock.get_frame_time()
                if start is None:
                    start = now
                elapsed = (now - start) / 1_000_000
                fraction = min(1, elapsed / duration)
                width = round(extent * 4 * fraction * (1 - fraction))
                if width != self.accent_width:
                    widget.queue_draw_area(0, 0, max(width, self.accent_width), 3)
                    self.accent_width = width
                if fraction >= 1:
                    self.tick_id = None
                    return False
                return True

            self.tick_id = self.area.add_tick_callback(advance)
        self.area.queue_draw()

    def wait(self, milliseconds):
        # GTK processes frame-clock events while the model waits for its next
        # update. SDL's timed wait would block the Wayland event loop.
        def wake(*_):
            self.loop.quit()
            return False

        timer = self.glib.timeout_source_new(milliseconds)
        timer.set_callback(wake)
        timer.attach()
        try:
            self.loop.run()
        finally:
            timer.destroy()
        return self.pygame.event.poll()

    def close(self):
        self.stop_animation()
        self.window.destroy()
