import { createReadStream } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";

const host = "127.0.0.1";
const port = Number.parseInt(process.env.PORT ?? "8766", 10);
const root = resolve("site-dist");
const headersFile = resolve(root, "_headers");
const notFoundFile = resolve(root, "404.html");

const mimeTypes = new Map([
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".svg", "image/svg+xml"],
  [".txt", "text/plain; charset=utf-8"],
  [".webp", "image/webp"]
]);

function parseHeaders(contents) {
  const entries = contents
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const separator = line.indexOf(":");
      if (separator < 1) {
        throw new Error(`Invalid header line: ${line}`);
      }
      return [line.slice(0, separator), line.slice(separator + 1).trim()];
    });

  return Object.fromEntries(entries);
}

const securityHeaders = parseHeaders(await readFile(headersFile, "utf8"));

function sendText(response, status, body) {
  response.writeHead(status, {
    ...securityHeaders,
    "Cache-Control": "no-store",
    "Content-Type": "text/plain; charset=utf-8"
  });
  response.end(body);
}

function isForbiddenPublicPath(relativePath) {
  if (relativePath === "_headers" || relativePath === ".htaccess") return true;
  return relativePath
    .replaceAll("\\", "/")
    .split("/")
    .some((segment) => segment.startsWith(".") && segment !== ".well-known");
}

async function sendNotFound(request, response) {
  try {
    const fileStat = await stat(notFoundFile);
    response.writeHead(404, {
      ...securityHeaders,
      "Cache-Control": "no-store",
      "Content-Length": fileStat.size,
      "Content-Type": "text/html; charset=utf-8"
    });
    if (request.method === "HEAD") {
      response.end();
      return;
    }
    createReadStream(notFoundFile).pipe(response);
  } catch {
    sendText(response, 404, "Not found");
  }
}

const server = createServer(async (request, response) => {
  try {
    if (request.method !== "GET" && request.method !== "HEAD") {
      response.setHeader("Allow", "GET, HEAD");
      sendText(response, 405, "Method not allowed");
      return;
    }

    const url = new URL(request.url ?? "/", `http://${host}:${port}`);
    const decodedPath = decodeURIComponent(url.pathname);
    const relativePath = decodedPath === "/" ? "index.html" : decodedPath.slice(1);
    const filePath = resolve(root, relativePath);

    if ((filePath !== root && !filePath.startsWith(`${root}${sep}`)) || isForbiddenPublicPath(relativePath)) {
      await sendNotFound(request, response);
      return;
    }

    const fileStat = await stat(filePath);
    if (!fileStat.isFile()) {
      await sendNotFound(request, response);
      return;
    }

    response.writeHead(200, {
      ...securityHeaders,
      "Cache-Control": "no-store",
      "Content-Length": fileStat.size,
      "Content-Type": mimeTypes.get(extname(filePath).toLowerCase()) ?? "application/octet-stream"
    });

    if (request.method === "HEAD") {
      response.end();
      return;
    }

    createReadStream(filePath).pipe(response);
  } catch (error) {
    if (error?.code === "ENOENT" || error instanceof URIError) {
      await sendNotFound(request, response);
      return;
    }
    console.error(error);
    sendText(response, 500, "Internal server error");
  }
});

server.listen(port, host, () => {
  console.log(`Secure website preview: http://${host}:${port}`);
});
