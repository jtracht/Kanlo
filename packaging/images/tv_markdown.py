"""Translate safe Markdown tokens into native display rows and load opt-in images."""
from collections import OrderedDict
from functools import lru_cache
import http.client
import io
import ipaddress
import queue
import socket
import threading
import time
from urllib.parse import urlsplit

try:
    from markdown_it import MarkdownIt
except ImportError:
    MarkdownIt = None

PARSER = MarkdownIt('commonmark', {'html': False, 'maxNesting': 16}).enable('table') if MarkdownIt else None
MAX_BYTES = 2 * 1024 * 1024
FORMATS = {'image/jpeg': 'JPEG', 'image/png': 'PNG', 'image/webp': 'WEBP'}


@lru_cache(maxsize=64)
def markdown_rows(markdown):
    if PARSER is None:
        return tuple(('text', row, None) for row in markdown.splitlines())
    rows, table, cells = [], False, []
    style, list_depth = 'text', 0
    for token in PARSER.parse(markdown):
        kind = token.type
        if kind == 'table_open':
            table = True
        elif kind == 'table_close':
            table = False
        elif kind == 'tr_close':
            rows.append(('table', '  │  '.join(cells), None))
            cells.clear()
        elif kind == 'heading_open':
            style = 'heading'
        elif kind == 'heading_close':
            style = 'text'
        elif kind == 'list_item_open':
            list_depth += 1
        elif kind == 'list_item_close':
            list_depth -= 1
        elif kind in ('fence', 'code_block'):
            rows.extend(('code', line, None) for line in token.content.splitlines())
        elif kind == 'inline':
            content = []
            images = []
            for child in token.children or []:
                if child.type == 'text':
                    content.append(child.content)
                elif child.type == 'code_inline':
                    content.append('‹' + child.content + '›')
                elif child.type in ('softbreak', 'hardbreak'):
                    content.append('\n')
                elif child.type == 'image':
                    alt = child.content or 'Bild'
                    content.append('[' + alt + ']')
                    if not table:
                        images.append(('image', alt, child.attrGet('src')))
                elif child.type == 'link_close':
                    content.append(' ↗')
            text = ''.join(content)
            if table:
                cells.append(text.replace('\n', ' '))
            else:
                for index, line in enumerate(text.splitlines()):
                    if index == 0 and list_depth:
                        line = '  ' * (list_depth - 1) + '• ' + line
                    rows.append((style, line, None))
                rows.extend(images)
    return tuple(rows)


def public_destination(url):
    if not url or len(url) > 2048 or any(ord(char) <= 32 or ord(char) == 127 for char in url):
        raise ValueError('Invalid image URL')
    parsed = urlsplit(url)
    if (parsed.scheme != 'https' or parsed.port not in (443, None) or not parsed.hostname
            or parsed.username is not None or parsed.password is not None or '\\' in url):
        raise ValueError('Images require a public HTTPS origin')
    host = parsed.hostname.encode('idna').decode('ascii')
    endpoints = socket.getaddrinfo(host, 443, socket.AF_INET, socket.SOCK_STREAM)
    addresses = {endpoint[4][0] for endpoint in endpoints}
    if not addresses or any(not ipaddress.ip_address(ip).is_global
                            or ipaddress.ip_address(ip).is_multicast for ip in addresses):
        raise ValueError('Image host resolves to a non-public address')
    target = parsed.path or '/'
    if parsed.query:
        target += '?' + parsed.query
    return host, sorted(addresses)[0], target


def fetch_image(url):
    from PIL import Image, PngImagePlugin

    PngImagePlugin.MAX_TEXT_CHUNK = 256 * 1024
    PngImagePlugin.MAX_TEXT_MEMORY = 1024 * 1024

    host, address, target = public_destination(url)
    connection = http.client.HTTPSConnection(host, timeout=3)
    # Use the checked address for TCP and the original hostname for TLS verification.
    connection._create_connection = lambda endpoint, timeout, source: socket.create_connection(
        (address, 443), timeout, source)
    deadline = time.monotonic() + 8
    try:
        connection.request('GET', target, headers={'Accept': ', '.join(FORMATS)})
        with connection.getresponse() as response:
            if response.status != 200 or response.headers.get('Content-Encoding', 'identity') != 'identity':
                raise ValueError('Invalid image response')
            image_format = FORMATS.get(response.headers.get_content_type())
            if not image_format or (response.length is not None and response.length > MAX_BYTES):
                raise ValueError('Unsupported image response')
            data = bytearray()
            while len(data) <= MAX_BYTES and time.monotonic() < deadline:
                chunk = response.read1(min(65536, MAX_BYTES + 1 - len(data)))
                if not chunk:
                    break
                data.extend(chunk)
            else:
                raise ValueError('Image download exceeded its budget')
    finally:
        connection.close()
    try:
        with Image.open(io.BytesIO(data), formats=[image_format]) as image:
            if image.width * image.height > 2_000_000 or getattr(image, 'n_frames', 1) != 1:
                raise ValueError('Image is too large or animated')
            image.thumbnail((480, 320))
            rgb = image.convert('RGB')
            return rgb.tobytes(), rgb.size
    except Image.DecompressionBombError as error:
        raise ValueError('Image exceeds the pixel budget') from error


class Images:
    def __init__(self, enabled):
        self.enabled = enabled
        self.jobs = queue.Queue(maxsize=4)
        self.results = queue.Queue(maxsize=4)
        self.pending = set()
        self.cache = OrderedDict()
        if enabled:
            threading.Thread(target=self.worker, daemon=True).start()

    def worker(self):
        while True:
            url = self.jobs.get()
            try:
                result = fetch_image(url)
            except (OSError, ValueError, ImportError, http.client.HTTPException):
                result = None
            self.results.put((url, result))

    def ready(self):
        changed = False
        while True:
            try:
                url, image = self.results.get_nowait()
            except queue.Empty:
                return changed
            self.pending.discard(url)
            self.cache[url] = image
            if len(self.cache) > 16:
                self.cache.popitem(last=False)
            changed = True

    def get(self, url):
        if not self.enabled or not url:
            return None
        if url in self.cache:
            self.cache.move_to_end(url)
            return self.cache[url]
        if url not in self.pending:
            try:
                self.jobs.put_nowait(url)
            except queue.Full:
                return None
            self.pending.add(url)
        return None
