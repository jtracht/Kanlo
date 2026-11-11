"""Publish a verified tag bundle on Gitea or GitHub using the job-local token."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import urllib.error
import urllib.parse
import urllib.request
import uuid

from release_package import ROOT, file_sha256, git, image_files, inspect, validate_tag


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class API:
    def __init__(self, server, base, repository, token):
        self.github = server == 'https://github.com'
        self.server, self.base, self.token = server.rstrip('/'), base.rstrip('/'), token
        if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
            raise ValueError('Invalid repository')
        expected = 'https://api.github.com' if self.github else self.server + '/api/v1'
        if self.base != expected or urllib.parse.urlparse(self.server).scheme != 'https' or not token:
            raise ValueError('Release API origin/token is not configured correctly')
        self.repo = f'{self.base}/repos/{repository}'
        self.opener = urllib.request.build_opener(NoRedirect())

    def request(self, method, url, body=None, content_type='application/json', binary=False, digest=False, length=None):
        if not url or url.startswith('/'):
            url = self.repo + url
        allowed = {urllib.parse.urlparse(self.base).netloc}
        if self.github:
            allowed.add('uploads.github.com')
        host = urllib.parse.urlparse(url)
        if host.scheme != 'https' or host.netloc not in allowed or host.username:
            raise ValueError('Refusing an untrusted authenticated URL')
        if isinstance(body, dict):
            body = json.dumps(body).encode()
        headers = {'Authorization': ('Bearer ' if self.github else 'token ') + self.token, 'Accept': 'application/octet-stream' if binary else 'application/json', 'Content-Type': content_type}
        if length is not None:
            headers['Content-Length'] = str(length)
        if self.github:
            headers['X-GitHub-Api-Version'] = '2026-03-10'
        for _ in range(5):
            try:
                with self.opener.open(urllib.request.Request(url, data=body, method=method, headers=headers), timeout=120) as response:
                    if binary and digest:
                        checksum = hashlib.sha256()
                        for chunk in iter(lambda: response.read(4 * 1024 * 1024), b''):
                            checksum.update(chunk)
                        return checksum.digest()
                    data = response.read()
                    return data if binary else json.loads(data) if data else None
            except urllib.error.HTTPError as error:
                if method == 'GET' and error.code in (301, 302, 303, 307, 308):
                    target = urllib.parse.urljoin(url, error.headers['Location'])
                    parsed = urllib.parse.urlparse(target)
                    trusted_asset = self.github and parsed.hostname and parsed.hostname.endswith('.githubusercontent.com')
                    if parsed.scheme != 'https' or parsed.username or parsed.netloc != host.netloc and not trusted_asset:
                        raise ValueError('Untrusted asset redirect') from None
                    if parsed.netloc != host.netloc:
                        headers = {'Accept': 'application/octet-stream'}
                    url = target
                    continue
                raise
        raise ValueError('Too many asset redirects')

    def download(self, release, asset, digest=False):
        if self.github:
            return self.request('GET', f"/releases/assets/{asset['id']}", binary=True, digest=digest)
        url = asset['browser_download_url']
        if urllib.parse.urlparse(url).netloc != urllib.parse.urlparse(self.server).netloc:
            raise ValueError('Untrusted Gitea asset URL')
        return self.request('GET', url, binary=True, digest=digest)

    def upload(self, release, path):
        def chunks(prefix=b'', suffix=b''):
            yield prefix
            with path.open('rb') as stream:
                yield from iter(lambda: stream.read(4 * 1024 * 1024), b'')
            yield suffix
        if self.github:
            url = release['upload_url'].split('{')[0] + '?name=' + urllib.parse.quote(path.name)
            return self.request('POST', url, chunks(), 'application/octet-stream', length=path.stat().st_size)
        boundary = uuid.uuid4().hex
        prefix = f'--{boundary}\r\nContent-Disposition: form-data; name="attachment"; filename="{path.name}"\r\nContent-Type: application/octet-stream\r\n\r\n'.encode()
        suffix = f'\r\n--{boundary}--\r\n'.encode()
        return self.request('POST', f"/releases/{release['id']}/assets?name={urllib.parse.quote(path.name)}", chunks(prefix, suffix), f'multipart/form-data; boundary={boundary}', length=len(prefix) + path.stat().st_size + len(suffix))


def checked_files(directory, tag):
    manifest = json.loads((directory / 'release-manifest.json').read_text())
    if manifest['tag'] != tag or manifest['source_commit'] != git('rev-parse', 'HEAD'):
        raise ValueError('Manifest does not belong to this tag/commit')
    files = []
    for line in (directory / 'SHA256SUMS').read_text().splitlines():
        checksum, name = line.split('  ', 1)
        if not re.fullmatch(r'[a-f0-9]{64}', checksum) or not re.fullmatch(r'[A-Za-z0-9._-]+', name):
            raise ValueError('Invalid checksum entry')
        path = directory / name
        if path.is_symlink() or file_sha256(path) != checksum:
            raise ValueError('Release checksum mismatch')
        files.append(path)
    expected = {f'kanlo-{tag[1:]}-linux-{arch}.tar.gz' for arch in ('amd64', 'arm64', 'armhf')} | {f'kanlo-{tag[1:]}-source.tar.gz', 'release-manifest.json'}
    image_assets, images = image_files(directory, manifest['source_commit'])
    expected.update(path.name for path in image_assets)
    if manifest.get('images') != images or len(files) != len(expected) or {p.name for p in files} != expected:
        raise ValueError('Exactly three packages, four verified images/evidence, source and manifest are required')
    for path in files:
        if '-linux-' in path.name:
            build = inspect(path)
            if build['source_commit'] != manifest['source_commit']:
                raise ValueError('Package source commit mismatch')
    return [*files, directory / 'SHA256SUMS']


def publish(api, tag, directory):
    prerelease = validate_tag(tag)
    commit = git('rev-parse', 'HEAD')
    if git('rev-parse', f'refs/tags/{tag}^{{commit}}') != commit:
        raise ValueError('Checkout is not the tagged commit')
    files = checked_files(directory, tag)
    repository = api.request('GET', '')
    if not repository['private'] and 'LICENSE REVIEW PENDING' in (ROOT / 'LICENSE').read_text():
        raise ValueError('Public distribution requires license clearance')
    try:
        release = api.request('GET', '/releases/tags/' + urllib.parse.quote(tag, safe=''))
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise
        release = api.request('POST', '/releases', {'tag_name': tag, 'target_commitish': commit, 'name': f'Kanlo {tag}', 'body': (ROOT / 'release/NOTES.md').read_text(), 'draft': True, 'prerelease': prerelease})
    if release['target_commitish'] != commit or bool(release['prerelease']) != prerelease:
        raise ValueError('Existing release has another commit or release channel')
    assets = api.request('GET', f"/releases/{release['id']}/assets")
    for path in files:
        existing = next((asset for asset in assets if asset['name'] == path.name), None)
        if existing is None:
            if not release['draft']:
                raise ValueError('Published releases are immutable; missing asset')
            existing = api.upload(release, path)
        digest = api.download(release, existing, digest=True)
        if digest.hex() != file_sha256(path):
            raise ValueError('Uploaded asset differs; refusing overwrite/publication')
    published_assets = api.request('GET', f"/releases/{release['id']}/assets")
    if len(published_assets) != len(files) or {asset['name'] for asset in published_assets} != {path.name for path in files}:
        raise ValueError('Unexpected or missing release attachments')
    if release['draft']:
        release = api.request('PATCH', f"/releases/{release['id']}", {'draft': False})
    return release['html_url']


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--directory', type=Path, required=True)
    args = parser.parse_args()
    try:
        api = API(os.environ['RELEASE_SERVER_URL'], os.environ['RELEASE_API_URL'], os.environ['RELEASE_REPOSITORY'], os.environ['RELEASE_TOKEN'])
        print(publish(api, os.environ['RELEASE_TAG'], args.directory))
    except urllib.error.HTTPError as error:
        raise SystemExit(f'Release API returned HTTP {error.code}; no credentials logged') from None
