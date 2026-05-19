<?php

declare(strict_types=1);

namespace HopTop\C12n;

use Composer\IO\IOInterface;
use Composer\Script\Event;

/**
 * Composer post-install hook that fetches `libc12n_core.{so,dylib,dll}`
 * from the matching c12n GitHub release, verifies its SHA256, and
 * extracts it into the vendor-local runtime cache.
 *
 * Wiring (set in composer.json):
 *
 *     "scripts": {
 *       "post-install-cmd": ["HopTop\\C12n\\Installer::download"],
 *       "post-update-cmd":  ["HopTop\\C12n\\Installer::download"]
 *     }
 *
 * Per ADR-0002 §4 the script:
 *   1. Reads `extra.c12n-core.version` to determine the release tag.
 *   2. Detects host OS + arch.
 *   3. Builds the asset URL from `extra.c12n-core.release-url-template`.
 *   4. Downloads the tarball + sibling `manifest.json`.
 *   5. SHA256-verifies the tarball against the manifest entry.
 *   6. Extracts into the package's `runtime/lib/` cache.
 *   7. Writes a `runtime/lib/.version` marker for idempotency.
 *
 * Skipped when:
 *   - `C12N_CORE_LIB_PATH` env var is set (airgapped / dev override —
 *     `Ffi::libPath()` will resolve the env var directly).
 *   - The cdylib already exists at the requested version (marker match).
 *
 * Failure mode: throw `\RuntimeException`. Composer surfaces the message
 * as an install error, which is the correct UX — silently continuing
 * with a half-installed package would yield cryptic FFI errors at first
 * `Pipeline::__construct` instead.
 */
final class Installer
{
    /** Canonical GitHub repo path. The tag lives here even if the
     *  mirror (hop-top/c12n) re-publishes the same release assets. */
    public const RELEASE_REPO = 'hop-top/poly-c12n';

    /** Manifest URL convention (sibling to the tarball asset). */
    public const MANIFEST_URL_TEMPLATE
        = 'https://github.com/' . self::RELEASE_REPO
        . '/releases/download/{tag}/manifest.json';

    /** User-Agent string sent on every HTTP request. */
    public const USER_AGENT = 'c12n-php-installer';

    /** HTTP timeout in seconds. */
    public const HTTP_TIMEOUT = 30;

    /**
     * Entry point wired to composer's post-install hook.
     *
     * @throws \RuntimeException on any non-recoverable failure
     */
    public static function download(Event $event): void
    {
        $io = $event->getIO();
        $composer = $event->getComposer();
        $extra = $composer->getPackage()->getExtra();

        if (self::shouldSkipForEnvOverride()) {
            $io->write('<info>c12n-php: C12N_CORE_LIB_PATH set, skipping download.</info>');
            return;
        }

        $version = self::readVersion($extra);
        $tag = self::tag($version);
        $template = self::readUrlTemplate($extra);

        $os = self::detectOs(\PHP_OS_FAMILY);
        $arch = self::detectArch(php_uname('m'));

        $packageRoot = self::resolvePackageRoot($composer);
        $libDir = $packageRoot . '/runtime/lib';
        $libExt = self::libExtensionFor($os);
        $libPath = $libDir . '/libc12n_core.' . $libExt;
        $marker = $libDir . '/.version';

        if (self::isInstalledAtVersion($libPath, $marker, $version)) {
            $io->write(sprintf(
                '<info>c12n-php: libc12n_core %s already installed at %s.</info>',
                $version,
                $libPath,
            ));
            return;
        }

        $assetUrl = self::buildAssetUrl($template, $tag, $os, $arch);
        $manifestUrl = self::buildManifestUrl($tag);
        $assetName = self::assetName($os, $arch);

        try {
            if (!is_dir($libDir) && !@mkdir($libDir, 0o755, true) && !is_dir($libDir)) {
                self::fail(sprintf('failed to create %s', $libDir));
            }

            $io->write(sprintf(
                '<info>c12n-php: Downloading libc12n_core %s for %s/%s from %s</info>',
                $version,
                $os,
                $arch,
                $assetUrl,
            ));
            $tarball = self::httpGet($assetUrl);

            $io->write('<info>c12n-php: Verifying SHA256 ...</info>');
            $manifest = self::httpGet($manifestUrl);
            $expected = self::expectedHash($manifest, $assetName);
            $actual = hash('sha256', $tarball);
            if (!hash_equals($expected, $actual)) {
                self::fail(sprintf(
                    'SHA256 mismatch for %s (expected %s, got %s)',
                    $assetName,
                    $expected,
                    $actual,
                ));
            }

            $tmpTarball = self::writeTempTarball($tarball);
            try {
                self::extractTarball($tmpTarball, $libDir, $libExt);
            } finally {
                @unlink($tmpTarball);
            }

            if (!is_file($libPath)) {
                self::fail(sprintf(
                    'extraction completed but %s is missing (tarball layout mismatch?)',
                    $libPath,
                ));
            }

            file_put_contents($marker, $version);

            $io->write(sprintf(
                '<info>c12n-php: Installed: %s</info>',
                self::relativeToCwd($libPath),
            ));
        } catch (\RuntimeException $e) {
            throw $e;
        } catch (\Throwable $e) {
            self::fail($e->getMessage());
        }
    }

    /**
     * Detect host OS family → release-asset key.
     *
     * Public so unit tests can drive it with arbitrary `PHP_OS_FAMILY`
     * values without spawning subprocesses.
     */
    public static function detectOs(string $osFamily): string
    {
        return match ($osFamily) {
            'Darwin' => 'macos',
            'Windows' => 'windows',
            'Linux', 'BSD', 'Solaris' => 'linux',
            default => self::unsupported(sprintf('unsupported OS family: %s', $osFamily)),
        };
    }

    /**
     * Normalize `php_uname('m')` machine names to the release-asset
     * arch keys (x86_64 / aarch64).
     */
    public static function detectArch(string $machine): string
    {
        $m = strtolower($machine);
        return match (true) {
            in_array($m, ['x86_64', 'amd64'], true) => 'x86_64',
            in_array($m, ['arm64', 'aarch64'], true) => 'aarch64',
            default => self::unsupported(sprintf('unsupported arch: %s', $machine)),
        };
    }

    /**
     * Compose the release-asset URL from the configured template.
     *
     * Substitutes `{tag}`, `{os}`, `{arch}` placeholders.
     */
    public static function buildAssetUrl(
        string $template,
        string $tag,
        string $os,
        string $arch,
    ): string {
        return strtr($template, [
            '{tag}' => $tag,
            '{os}' => $os,
            '{arch}' => $arch,
        ]);
    }

    /**
     * Build the manifest URL for a given tag (sibling release asset).
     */
    public static function buildManifestUrl(string $tag): string
    {
        return strtr(self::MANIFEST_URL_TEMPLATE, ['{tag}' => $tag]);
    }

    /**
     * Construct the release tag from a cdylib version.
     *
     * Linked-versions per ADR-0002 §5: the c12n-core slice of the
     * monorepo tags as `c12n-core/v<version>`.
     */
    public static function tag(string $version): string
    {
        return 'c12n-core/v' . $version;
    }

    /**
     * Release-asset filename for the given OS / arch.
     */
    public static function assetName(string $os, string $arch): string
    {
        return sprintf('libc12n_core-%s-%s.tar.gz', $os, $arch);
    }

    /**
     * Map an OS key to the corresponding cdylib extension.
     */
    public static function libExtensionFor(string $os): string
    {
        return match ($os) {
            'macos' => 'dylib',
            'windows' => 'dll',
            default => 'so',
        };
    }

    /**
     * Resolve `extra.c12n-core.version`. Throws on missing/blank.
     *
     * @param array<string,mixed> $extra
     */
    public static function readVersion(array $extra): string
    {
        $version = $extra['c12n-core']['version'] ?? null;
        if (!is_string($version) || $version === '') {
            self::fail('composer.json missing extra.c12n-core.version');
        }
        return $version;
    }

    /**
     * Resolve `extra.c12n-core.release-url-template`. Throws on missing/blank.
     *
     * @param array<string,mixed> $extra
     */
    public static function readUrlTemplate(array $extra): string
    {
        $tpl = $extra['c12n-core']['release-url-template'] ?? null;
        if (!is_string($tpl) || $tpl === '') {
            self::fail('composer.json missing extra.c12n-core.release-url-template');
        }
        return $tpl;
    }

    /**
     * Look up the expected SHA256 hex digest for an asset from a
     * manifest JSON payload.
     *
     * Manifest shape: `{"<asset-filename>": "sha256:<hex>"}` or just
     * `{"<asset-filename>": "<hex>"}` — both forms accepted.
     */
    public static function expectedHash(string $manifestJson, string $assetName): string
    {
        try {
            $decoded = json_decode($manifestJson, true, flags: JSON_THROW_ON_ERROR);
        } catch (\JsonException $e) {
            self::fail('manifest.json is not valid JSON: ' . $e->getMessage());
        }
        if (!is_array($decoded) || !isset($decoded[$assetName]) || !is_string($decoded[$assetName])) {
            self::fail(sprintf(
                'manifest.json missing SHA256 entry for %s',
                $assetName,
            ));
        }
        $raw = $decoded[$assetName];
        if (str_starts_with($raw, 'sha256:')) {
            $raw = substr($raw, 7);
        }
        $raw = strtolower(trim($raw));
        if (!preg_match('/^[0-9a-f]{64}$/', $raw)) {
            self::fail(sprintf(
                'manifest.json SHA256 entry for %s is not a hex digest',
                $assetName,
            ));
        }
        return $raw;
    }

    /** Skip when `C12N_CORE_LIB_PATH` is set non-empty. */
    private static function shouldSkipForEnvOverride(): bool
    {
        $env = getenv('C12N_CORE_LIB_PATH');
        return is_string($env) && $env !== '';
    }

    /**
     * The cdylib at $libPath is treated as up-to-date if both the
     * binary and the `.version` marker exist and match $version.
     */
    private static function isInstalledAtVersion(
        string $libPath,
        string $marker,
        string $version,
    ): bool {
        if (!is_file($libPath) || !is_file($marker)) {
            return false;
        }
        $existing = @file_get_contents($marker);
        return is_string($existing) && trim($existing) === $version;
    }

    /**
     * Locate the c12n-php package install root via Composer's
     * InstallationManager. Falls back to the source-tree root when
     * Composer is operating on the c12n-php repo itself (no install
     * path because the package is its own root).
     */
    private static function resolvePackageRoot(\Composer\Composer $composer): string
    {
        $rootPackage = $composer->getPackage();
        if ($rootPackage->getName() === 'hop-top/c12n-php' || $rootPackage->getName() === 'hop-top/c12n') {
            // Hook fired against the package's own composer.json (dev
            // checkout). `__DIR__` parent is the package root.
            return dirname(__DIR__);
        }

        $installManager = $composer->getInstallationManager();
        $repo = $composer->getRepositoryManager()->getLocalRepository();
        foreach ($repo->getPackages() as $pkg) {
            if (in_array($pkg->getName(), ['hop-top/c12n-php', 'hop-top/c12n'], true)) {
                return $installManager->getInstallPath($pkg);
            }
        }
        // Last-resort fallback — should never hit in practice.
        return dirname(__DIR__);
    }

    /**
     * `file_get_contents` with a 30s timeout and a User-Agent header.
     *
     * Returns the raw response body. Throws on any non-2xx / network
     * error. PHP's stream wrapper surfaces HTTP status via
     * `$http_response_header`.
     */
    private static function httpGet(string $url): string
    {
        $ctx = stream_context_create([
            'http' => [
                'method' => 'GET',
                'header' => 'User-Agent: ' . self::USER_AGENT . "\r\n"
                    . 'Accept: application/octet-stream' . "\r\n",
                'timeout' => self::HTTP_TIMEOUT,
                'follow_location' => 1,
                'max_redirects' => 5,
                'ignore_errors' => true,
            ],
        ]);

        $body = @file_get_contents($url, false, $ctx);
        if ($body === false) {
            $err = error_get_last();
            self::fail(sprintf(
                'HTTP GET failed for %s: %s',
                $url,
                $err['message'] ?? 'unknown error',
            ));
        }

        // PHP 8.5+ exposes response headers via http_get_last_response_headers();
        // older versions populate the local-scope $http_response_header.
        $headers = function_exists('http_get_last_response_headers')
            ? http_get_last_response_headers()
            : ($http_response_header ?? null);
        if (is_array($headers)) {
            $status = self::parseStatus($headers);
            if ($status !== null && ($status < 200 || $status >= 300)) {
                self::fail(sprintf('HTTP %d for %s', $status, $url));
            }
        }

        return $body;
    }

    /**
     * @param array<int,string> $headers
     */
    private static function parseStatus(array $headers): ?int
    {
        foreach ($headers as $h) {
            if (preg_match('#^HTTP/\S+\s+(\d{3})#', $h, $m)) {
                return (int) $m[1];
            }
        }
        return null;
    }

    /**
     * Persist downloaded tarball to a temp file before extracting.
     * PharData (preferred extractor) operates on file paths, not
     * in-memory strings.
     */
    private static function writeTempTarball(string $data): string
    {
        $tmp = tempnam(sys_get_temp_dir(), 'c12n-core-') . '.tar.gz';
        if (@file_put_contents($tmp, $data) === false) {
            self::fail('failed to write tarball to temp file');
        }
        return $tmp;
    }

    /**
     * Extract the cdylib from a gzipped tarball into $libDir.
     *
     * Strategy: prefer `PharData` (built-in, no shell-out, works on
     * Windows). Fall back to `tar` if Phar's tar reader rejects the
     * archive — defensive for tarballs produced with unusual flags.
     */
    private static function extractTarball(string $tarball, string $libDir, string $libExt): void
    {
        $expectedFile = 'libc12n_core.' . $libExt;

        try {
            $phar = new \PharData($tarball);
            // Locate the file within the archive — accept it whether
            // it lives at the root or in a one-level prefix dir.
            $found = self::findInPhar($phar, $expectedFile);
            if ($found === null) {
                self::fail(sprintf(
                    'tarball does not contain %s',
                    $expectedFile,
                ));
            }
            $phar->extractTo($libDir, $found, overwrite: true);

            // If the file was nested, hoist it to $libDir/$expectedFile.
            $extractedPath = $libDir . '/' . $found;
            $finalPath = $libDir . '/' . $expectedFile;
            if ($extractedPath !== $finalPath) {
                if (!@rename($extractedPath, $finalPath)) {
                    self::fail(sprintf(
                        'failed to move %s → %s',
                        $extractedPath,
                        $finalPath,
                    ));
                }
            }
        } catch (\UnexpectedValueException | \BadMethodCallException $e) {
            self::fail('failed to extract tarball: ' . $e->getMessage());
        }
    }

    /**
     * Walk a PharData archive and return the relative path matching
     * the expected filename, or null when absent.
     */
    private static function findInPhar(\PharData $phar, string $expectedFile): ?string
    {
        $it = new \RecursiveIteratorIterator($phar);
        foreach ($it as $file) {
            if ($file->getFilename() === $expectedFile) {
                $rel = ltrim(str_replace($phar->getPath(), '', (string) $file), '/');
                // Drop the "phar://<tarball>/" prefix that the iterator
                // includes on some platforms.
                $rel = preg_replace('#^phar://[^/]+/#', '', $rel) ?? $rel;
                return $rel;
            }
        }
        return null;
    }

    /** Render a path relative to CWD for cleaner installer output. */
    private static function relativeToCwd(string $path): string
    {
        $cwd = getcwd();
        if ($cwd !== false && str_starts_with($path, $cwd . '/')) {
            return substr($path, strlen($cwd) + 1);
        }
        return $path;
    }

    /**
     * Throw a uniformly-prefixed RuntimeException with the override
     * hint appended for actionable error UX.
     *
     * @return never
     */
    private static function fail(string $what): void
    {
        throw new \RuntimeException(sprintf(
            'c12n-php: %s; set C12N_CORE_LIB_PATH to bypass the installer '
            . 'if you have libc12n_core built locally',
            $what,
        ));
    }

    /**
     * Internal helper: same as fail() but conveys "unsupported host"
     * intent. Identical exception type/shape — separated only so the
     * call sites in match() arms read cleanly.
     *
     * @return never
     */
    private static function unsupported(string $what): string
    {
        self::fail($what);
    }
}
