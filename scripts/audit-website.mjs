import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const websiteRoot = path.join(repositoryRoot, "website");
const outputRoot = path.join(repositoryRoot, "site-dist");
const sourceHtmlPaths = [
  path.join(websiteRoot, "index.html"),
  path.join(websiteRoot, "security.html"),
  path.join(websiteRoot, "support.html"),
  path.join(repositoryRoot, "fluxdrop-website.html")
];
const sourceScriptPaths = [path.join(websiteRoot, "assets", "site.js")];
const sourceStylePaths = [path.join(websiteRoot, "assets", "styles.css")];
const sourceSvgPaths = [path.join(websiteRoot, "assets", "favicon.svg")];

const requiredHeaderNames = [
  "Content-Security-Policy",
  "Cross-Origin-Embedder-Policy",
  "Cross-Origin-Opener-Policy",
  "Cross-Origin-Resource-Policy",
  "Origin-Agent-Cluster",
  "Permissions-Policy",
  "Referrer-Policy",
  "Strict-Transport-Security",
  "X-Content-Type-Options",
  "X-DNS-Prefetch-Control",
  "X-Frame-Options",
  "X-Permitted-Cross-Domain-Policies"
];

const requiredCspDirectives = [
  "default-src 'none'",
  "script-src 'self'",
  "script-src-attr 'none'",
  "style-src 'self'",
  "style-src-attr 'unsafe-inline'",
  "img-src 'self'",
  "connect-src 'none'",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
  "frame-ancestors 'none'",
  "require-trusted-types-for 'script'",
  "trusted-types 'none'"
];

const failures = [];

function displayPath(filePath) {
  return path.relative(repositoryRoot, filePath).replaceAll("\\", "/");
}

function fail(filePath, message) {
  failures.push(`${displayPath(filePath)}: ${message}`);
}

async function read(filePath) {
  return readFile(filePath, "utf8");
}

function auditHtml(filePath, html, { requireSitePolicy }) {
  if (/<!--[\s\S]*?-->/.test(html)) fail(filePath, "HTML comments are forbidden");
  if (/<script\b(?![^>]*\bsrc\s*=)[^>]*>/i.test(html)) {
    fail(filePath, "inline script blocks are forbidden");
  }
  if (/\son[a-z]+\s*=/i.test(html)) fail(filePath, "inline event handlers are forbidden");
  if (/\sstyle\s*=/i.test(html)) fail(filePath, "inline style attributes are forbidden");
  if (/(?:href|src)\s*=\s*["']\s*(?:javascript:|data:text\/html)/i.test(html)) {
    fail(filePath, "unsafe URL scheme found");
  }
  if (/<(?:iframe|object|embed|base|form)\b/i.test(html)) {
    fail(filePath, "disallowed active-content or submission element found");
  }

  const runtimeAssetPattern =
    /<(?:script|img|link|source)\b[^>]*(?:src|href)\s*=\s*["']([^"']+)["'][^>]*>/gi;
  for (const match of html.matchAll(runtimeAssetPattern)) {
    if (/^(?:https?:)?\/\//i.test(match[1])) {
      fail(filePath, `external runtime asset found: ${match[1]}`);
    }
  }

  for (const match of html.matchAll(/<a\b[^>]*\btarget\s*=\s*["']_blank["'][^>]*>/gi)) {
    const rel = match[0].match(/\brel\s*=\s*["']([^"']+)["']/i)?.[1] ?? "";
    const tokens = new Set(rel.toLowerCase().split(/\s+/).filter(Boolean));
    if (!tokens.has("noopener") || !tokens.has("noreferrer")) {
      fail(filePath, 'target="_blank" link must include rel="noopener noreferrer"');
    }
  }

  if (!/http-equiv\s*=\s*["']Content-Security-Policy["']/i.test(html)) {
    fail(filePath, "missing meta Content Security Policy");
  }
  if (!/name\s*=\s*["']referrer["'][^>]*content\s*=\s*["']no-referrer["']/i.test(html)) {
    fail(filePath, "missing no-referrer meta policy");
  }

  if (requireSitePolicy) {
    if (!/<script\b[^>]*type\s*=\s*["']module["'][^>]*src\s*=\s*["'][^"']+["']/i.test(html)) {
      fail(filePath, "missing external module script");
    }
    if (!/require-trusted-types-for 'script'/.test(html) || !/trusted-types 'none'/.test(html)) {
      fail(filePath, "meta CSP must enforce Trusted Types");
    }
  }
}

function auditJavaScript(filePath, source) {
  const dangerousPatterns = [
    [/\b(?:innerHTML|outerHTML)\b/, "HTML parsing sink"],
    [/\binsertAdjacentHTML\s*\(/, "insertAdjacentHTML sink"],
    [/\bdocument\.write(?:ln)?\s*\(/, "document.write sink"],
    [/\beval\s*\(/, "eval usage"],
    [/\bnew\s+Function\b/, "Function constructor"],
    [/\bset(?:Timeout|Interval)\s*\(\s*["'`]/, "string-based timer execution"],
    [/\b(?:fetch|XMLHttpRequest|WebSocket|EventSource|sendBeacon)\b/, "outbound network API"]
  ];

  for (const [pattern, label] of dangerousPatterns) {
    if (pattern.test(source)) fail(filePath, `${label} is forbidden`);
  }
}

function auditCss(filePath, source) {
  const dangerousPatterns = [
    [/@import\b/i, "CSS imports"],
    [/url\(\s*["']?\s*(?:https?:)?\/\//i, "external CSS URL"],
    [/\bexpression\s*\(/i, "CSS expression"],
    [/(?:^|[;{])\s*behavior\s*:/im, "legacy CSS behavior"],
    [/-moz-binding\s*:/i, "legacy XBL binding"]
  ];

  for (const [pattern, label] of dangerousPatterns) {
    if (pattern.test(source)) fail(filePath, `${label} are forbidden`);
  }
}

function auditSvg(filePath, source) {
  if (/<script\b|<foreignObject\b|\son[a-z]+\s*=|(?:href|src)\s*=\s*["']https?:/i.test(source)) {
    fail(filePath, "active content or external resources found in SVG");
  }
}

for (const filePath of sourceHtmlPaths) {
  auditHtml(filePath, await read(filePath), {
    requireSitePolicy: filePath.startsWith(websiteRoot)
  });
}

for (const filePath of sourceScriptPaths) auditJavaScript(filePath, await read(filePath));
for (const filePath of sourceStylePaths) auditCss(filePath, await read(filePath));
for (const filePath of sourceSvgPaths) auditSvg(filePath, await read(filePath));

const outputHtmlPaths = (await readdir(outputRoot))
  .filter((fileName) => fileName.endsWith(".html"))
  .map((fileName) => path.join(outputRoot, fileName));

for (const filePath of outputHtmlPaths) {
  auditHtml(filePath, await read(filePath), { requireSitePolicy: true });
}

const outputAssetsRoot = path.join(outputRoot, "assets");
for (const fileName of await readdir(outputAssetsRoot)) {
  const filePath = path.join(outputAssetsRoot, fileName);
  if (fileName.endsWith(".js")) auditJavaScript(filePath, await read(filePath));
  if (fileName.endsWith(".css")) auditCss(filePath, await read(filePath));
  if (fileName.endsWith(".svg")) auditSvg(filePath, await read(filePath));
}

const headersSourcePath = path.join(websiteRoot, "public", "_headers");
const builtHeadersPath = path.join(outputRoot, "_headers");
const vercelConfigPath = path.join(repositoryRoot, "vercel.json");
const securityTextPath = path.join(outputRoot, ".well-known", "security.txt");
const headersSource = await read(headersSourcePath);
const builtHeaders = await read(builtHeadersPath);
const vercelConfig = JSON.parse(await read(vercelConfigPath));

for (const headerName of requiredHeaderNames) {
  if (!headersSource.includes(`${headerName}:`)) fail(headersSourcePath, `missing ${headerName}`);
  if (!builtHeaders.includes(`${headerName}:`)) fail(builtHeadersPath, `missing ${headerName}`);
}

for (const directive of requiredCspDirectives) {
  if (!headersSource.includes(directive)) fail(headersSourcePath, `CSP missing ${directive}`);
}

const vercelHeaders = new Map(
  (vercelConfig.headers?.[0]?.headers ?? []).map((header) => [header.key, header.value])
);
for (const headerName of requiredHeaderNames) {
  if (!vercelHeaders.has(headerName)) fail(vercelConfigPath, `missing ${headerName}`);
}
for (const directive of requiredCspDirectives) {
  if (!vercelHeaders.get("Content-Security-Policy")?.includes(directive)) {
    fail(vercelConfigPath, `CSP missing ${directive}`);
  }
}

const securityText = await read(securityTextPath);
if (!/^Contact: https:\/\/github\.com\/savxzthc\/FluxDrop\/security\/advisories\/new$/m.test(securityText)) {
  fail(securityTextPath, "missing private vulnerability-reporting contact");
}
if (!/^Expires: \d{4}-\d{2}-\d{2}T/m.test(securityText)) {
  fail(securityTextPath, "missing expiration date");
}

if (failures.length > 0) {
  console.error(`Website security audit failed with ${failures.length} finding(s):`);
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Website security audit passed: ${sourceHtmlPaths.length} source pages, ${outputHtmlPaths.length} built pages, no HTML comments, no executable inline code, no external runtime assets, and hardened deployment policies present.`
);
