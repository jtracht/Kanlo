"""Present the software-rendered Pygame board through GTK's Wayland surface."""
class WaylandDisplay:
    def __init__(self, pygame, size, on_frame):
        import gi
        gi.require_version('Gtk', '3.0')
        gi.require_version('Gdk', '3.0')
        from gi.repository import Gdk, GdkPixbuf, GLib, Gtk

        self.pygame = pygame
        self.gtk = Gtk
        self.gdk = Gdk
        self.pixbuf = GdkPixbuf
        self.glib = GLib
        self.on_frame = on_frame
        self.frame = None
        self.surface = pygame.Surface(size)
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
            self.surface = self.pygame.Surface(size)

    def draw(self, _, context):
        if self.frame is not None:
            self.gdk.cairo_set_source_pixbuf(context, self.frame, 0, 0)
            context.paint()
            self.on_frame()
        return False

    def present(self):
        width, height = self.surface.get_size()
        pixels = self.glib.Bytes.new(self.pygame.image.tobytes(self.surface, 'RGB'))
        self.frame = self.pixbuf.Pixbuf.new_from_bytes(
            pixels, self.pixbuf.Colorspace.RGB, False, 8, width, height, width * 3,
        )
        self.area.queue_draw()

    def pump(self):
        while self.gtk.events_pending():
            self.gtk.main_iteration_do(False)

    def close(self):
        self.window.destroy()
