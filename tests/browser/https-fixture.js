import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { createHash, X509Certificate } from "node:crypto";
import { createServer } from "node:https";
import { request } from "node:http";
import { pathToFileURL } from "node:url";

export function certificate() {
  const directory = resolve(import.meta.dirname, "../../data/https-fixture");
  mkdirSync(directory, { recursive: true });
  const key = resolve(directory, "key.pem"),
    cert = resolve(directory, "cert.pem");
  if (!existsSync(cert)) {
    const openssl =
      process.platform === "win32"
        ? "C:/Program Files/Git/usr/bin/openssl.exe"
        : "openssl";
    execFileSync(
      openssl,
      [
        "req",
        "-x509",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-keyout",
        key,
        "-out",
        cert,
        "-days",
        "2",
        "-subj",
        "/CN=kanlo.test",
        "-addext",
        "subjectAltName=DNS:kanlo.test",
      ],
      { stdio: "pipe" },
    );
  }
  const spki = new X509Certificate(readFileSync(cert)).publicKey.export({
    type: "spki",
    format: "der",
  });
  return {
    key: readFileSync(key),
    cert: readFileSync(cert),
    pin: createHash("sha256").update(spki).digest("base64"),
  };
}
if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const server = createServer(certificate(), (incoming, outgoing) => {
    const proxy = request(
      {
        host: "127.0.0.1",
        port: 18081,
        path: incoming.url,
        method: incoming.method,
        headers: incoming.headers,
      },
      (response) => {
        outgoing.writeHead(response.statusCode, response.headers);
        // SSE headers must reach the browser before the first event/keepalive.
        outgoing.flushHeaders();
        response.pipe(outgoing);
      },
    );
    proxy.on("error", () => {
      outgoing.writeHead(502);
      outgoing.end();
    });
    incoming.pipe(proxy);
    outgoing.on("close", () => proxy.destroy());
  });
  server.listen(18443, "127.0.0.1");
  process.on("SIGTERM", () => server.close(() => process.exit()));
}
