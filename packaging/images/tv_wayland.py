"""Present the software-rendered Pygame board through GTK's Wayland surface."""

RGB24_MASKS = (0x00FF0000, 0x0000FF00, 0x000000FF, 0)


class WaylandDisplay:
    def __init__(self, pygame, size, on_frame):
        import gi
        gi.require_version('Gtk', '3.0')
        gi.require_version('Gdk', '3.0')
        import cairo
        from gi.repository import Gdk, Gtk

        self.pygame = pygame
        self.gtk = Gtk
        self.gdk = Gdk
        self.cairo = cairo
        self.on_frame = on_frame
        self.frame = None
        self.previous = None
        self.tick_id = None
        self.progress = 1
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
        return True

    def key(self, _, event):
        keys = {self.gdk.KEY_Left: self.pygame.K_LEFT, self.gdk.KEY_Right: self.pygame.K_RIGHT,
                self.gdk.KEY_space: self.pygame.K_SPACE, self.gdk.KEY_Escape: self.pygame.K_ESCAPE}
        if event.keyval in keys:
            self.pygame.event.post(self.pygame.event.Event(self.pygame.KEYDOWN, key=keys[event.keyval]))
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
        if self.previous is None:
            context.set_source_surface(self.frame)
            context.paint()
        else:
            width, height = self.surface.get_size()
            offset = round(width * self.progress)
            boundary = width - offset
            # Both rectangles are opaque and disjoint. Pixman only copies
            # each output pixel once, even when the entire board moves.
            context.set_source_surface(self.previous, -offset, 0)
            context.rectangle(0, 0, boundary, height)
            context.fill()
            context.set_source_surface(self.frame, boundary, 0)
            context.rectangle(boundary, 0, offset, height)
            context.fill()
        self.on_frame()
        return False

    def stop_animation(self):
        if self.tick_id is not None:
            self.area.remove_tick_callback(self.tick_id)
            self.tick_id = None
        self.previous = None
        self.progress = 1

    def present(self, duration=0):
        self.stop_animation()
        width, height = self.surface.get_size()
        # Both surfaces use native-endian 0x00RRGGBB. Copy once so Pygame can
        # draw the next scene while GTK still presents this one.
        pixels = bytearray(self.surface.get_view('0'))
        previous = self.frame
        self.frame = self.cairo.ImageSurface.create_for_data(
            pixels, self.cairo.FORMAT_RGB24, width, height, self.surface.get_pitch(),
        )
        if duration and previous is not None:
            self.previous, self.progress = previous, 0
            start = None

            def advance(widget, clock):
                nonlocal start
                now = clock.get_frame_time()
                if start is None:
                    start = now
                elapsed = (now - start) / 1_000_000
                fraction = max(0, min(1, elapsed / duration))
                self.progress = fraction * fraction * (3 - 2 * fraction)
                widget.queue_draw()
                if fraction >= 1:
                    self.tick_id = self.previous = None
                    return False
                return True

            self.tick_id = self.area.add_tick_callback(advance)
        self.area.queue_draw()

    def pump(self):
        while self.gtk.events_pending():
            self.gtk.main_iteration_do(False)

    def close(self):
        self.stop_animation()
        self.window.destroy()
