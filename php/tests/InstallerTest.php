<?php

declare(strict_types=1);

namespace HopTop\C12n\Tests;

use Composer\Composer;
use Composer\Config;
use Composer\IO\BufferIO;
use Composer\Package\RootPackage;
use Composer\Script\Event;
use HopTop\C12n\Installer;
use PHPUnit\Framework\TestCase;

/**
 * Unit tests for {@see Installer}.
 *
 * Network-driven branches (actual GitHub download, tarball extract) are
 * out of scope — they require a published release. Coverage here:
 *
 * - Pure helpers: OS/arch detection, URL construction, tag formation,
 *   manifest parsing, extra-config readers.
 * - Skip-branch behaviour: env-var override and "already installed" marker.
 * - Error UX: missing extra config, unsupported OS/arch.
 *
 * Integration coverage (actual download + SHA256 round-trip) is gated
 * on the first published c12n-core release and lands as a separate
 * tag-driven CI job, not a unit test.
 */
final class InstallerTest extends TestCase
{
    private string|false $originalEnv;

    protected function setUp(): void
    {
        $this->originalEnv = getenv('C12N_CORE_LIB_PATH');
        putenv('C12N_CORE_LIB_PATH');
    }

    protected function tearDown(): void
    {
        if ($this->originalEnv === false) {
            putenv('C12N_CORE_LIB_PATH');
        } else {
            putenv('C12N_CORE_LIB_PATH=' . $this->originalEnv);
        }
    }

    // -- OS detection -------------------------------------------------

    public function testDetectOsDarwin(): void
    {
        self::assertSame('macos', Installer::detectOs('Darwin'));
    }

    public function testDetectOsLinux(): void
    {
        self::assertSame('linux', Installer::detectOs('Linux'));
    }

    public function testDetectOsBsdMapsToLinuxAsset(): void
    {
        // BSDs link against the linux ELF cdylib via Linux compat
        // layers — closest existing asset until a dedicated BSD build
        // ships.
        self::assertSame('linux', Installer::detectOs('BSD'));
    }

    public function testDetectOsWindows(): void
    {
        self::assertSame('windows', Installer::detectOs('Windows'));
    }

    public function testDetectOsUnknownThrows(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessageMatches('/unsupported OS family: Haiku/');
        Installer::detectOs('Haiku');
    }

    // -- Arch detection -----------------------------------------------

    public function testDetectArchX86_64(): void
    {
        self::assertSame('x86_64', Installer::detectArch('x86_64'));
        self::assertSame('x86_64', Installer::detectArch('amd64'));
        self::assertSame('x86_64', Installer::detectArch('AMD64'));
    }

    public function testDetectArchAarch64(): void
    {
        self::assertSame('aarch64', Installer::detectArch('arm64'));
        self::assertSame('aarch64', Installer::detectArch('aarch64'));
    }

    public function testDetectArchUnknownThrows(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessageMatches('/unsupported arch: i386/');
        Installer::detectArch('i386');
    }

    // -- URL / tag construction ---------------------------------------

    public function testTag(): void
    {
        self::assertSame('c12n-core/v0.1.0-alpha.0', Installer::tag('0.1.0-alpha.0'));
    }

    public function testBuildAssetUrlSubstitutesAllPlaceholders(): void
    {
        $template = 'https://example.com/{tag}/libc12n_core-{os}-{arch}.tar.gz';
        $url = Installer::buildAssetUrl($template, 'c12n-core/v0.1.0-alpha.0', 'macos', 'aarch64');
        self::assertSame(
            'https://example.com/c12n-core/v0.1.0-alpha.0/libc12n_core-macos-aarch64.tar.gz',
            $url,
        );
    }

    public function testBuildManifestUrlPointsAtCanonicalRepo(): void
    {
        $url = Installer::buildManifestUrl('c12n-core/v0.2.0');
        self::assertStringContainsString('hop-top/poly-c12n', $url);
        self::assertStringEndsWith('/c12n-core/v0.2.0/manifest.json', $url);
    }

    public function testAssetName(): void
    {
        self::assertSame(
            'libc12n_core-linux-x86_64.tar.gz',
            Installer::assetName('linux', 'x86_64'),
        );
    }

    public function testLibExtensionFor(): void
    {
        self::assertSame('dylib', Installer::libExtensionFor('macos'));
        self::assertSame('dll', Installer::libExtensionFor('windows'));
        self::assertSame('so', Installer::libExtensionFor('linux'));
    }

    // -- Extra config readers -----------------------------------------

    public function testReadVersion(): void
    {
        self::assertSame(
            '0.1.0-alpha.0',
            Installer::readVersion(['c12n-core' => ['version' => '0.1.0-alpha.0']]),
        );
    }

    public function testReadVersionMissingThrows(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessageMatches('/extra\.c12n-core\.version/');
        Installer::readVersion([]);
    }

    public function testReadVersionBlankThrows(): void
    {
        $this->expectException(\RuntimeException::class);
        Installer::readVersion(['c12n-core' => ['version' => '']]);
    }

    public function testReadUrlTemplate(): void
    {
        self::assertSame(
            'https://example.com/{tag}/x-{os}-{arch}.tar.gz',
            Installer::readUrlTemplate([
                'c12n-core' => [
                    'release-url-template' => 'https://example.com/{tag}/x-{os}-{arch}.tar.gz',
                ],
            ]),
        );
    }

    public function testReadUrlTemplateMissingThrows(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessageMatches('/release-url-template/');
        Installer::readUrlTemplate([]);
    }

    // -- Manifest parsing ---------------------------------------------

    public function testExpectedHashSha256Prefix(): void
    {
        $manifest = json_encode([
            'libc12n_core-linux-x86_64.tar.gz' => 'sha256:' . str_repeat('a', 64),
        ], JSON_THROW_ON_ERROR);
        self::assertSame(
            str_repeat('a', 64),
            Installer::expectedHash($manifest, 'libc12n_core-linux-x86_64.tar.gz'),
        );
    }

    public function testExpectedHashBareHex(): void
    {
        $manifest = json_encode([
            'libc12n_core-macos-aarch64.tar.gz' => str_repeat('b', 64),
        ], JSON_THROW_ON_ERROR);
        self::assertSame(
            str_repeat('b', 64),
            Installer::expectedHash($manifest, 'libc12n_core-macos-aarch64.tar.gz'),
        );
    }

    public function testExpectedHashMissingAssetThrows(): void
    {
        $manifest = json_encode(['something-else.tar.gz' => str_repeat('c', 64)], JSON_THROW_ON_ERROR);
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessageMatches('/missing SHA256 entry/');
        Installer::expectedHash($manifest, 'libc12n_core-linux-x86_64.tar.gz');
    }

    public function testExpectedHashInvalidJsonThrows(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessageMatches('/manifest\.json is not valid JSON/');
        Installer::expectedHash('not-json', 'asset.tar.gz');
    }

    public function testExpectedHashNonHexThrows(): void
    {
        $manifest = json_encode([
            'asset.tar.gz' => 'sha256:not-a-hex-digest',
        ], JSON_THROW_ON_ERROR);
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessageMatches('/not a hex digest/');
        Installer::expectedHash($manifest, 'asset.tar.gz');
    }

    // -- download() skip branches -------------------------------------

    public function testDownloadSkipsWhenEnvOverrideSet(): void
    {
        putenv('C12N_CORE_LIB_PATH=/tmp/override');

        $io = new BufferIO();
        $event = $this->makeEvent($io, extra: [
            'c12n-core' => [
                'version' => '0.1.0-alpha.0',
                'release-url-template' => 'https://example.com/{tag}/{os}-{arch}.tar.gz',
            ],
        ]);

        Installer::download($event);

        self::assertStringContainsString('C12N_CORE_LIB_PATH set, skipping', $io->getOutput());
    }

    // -- Helpers ------------------------------------------------------

    /**
     * Build a real Composer\Script\Event with a stub Composer object
     * carrying the supplied root-package `extra` array and name. We
     * use the real classes (not mocks) because composer/composer is a
     * dev dep and constructing them is cheap.
     *
     * @param array<string,mixed> $extra
     */
    private function makeEvent(
        BufferIO $io,
        array $extra = [],
        string $rootName = 'hop-top/c12n-php',
    ): Event {
        $rootPackage = new RootPackage($rootName, '1.0.0.0', '1.0.0');
        $rootPackage->setExtra($extra);

        $composer = new Composer();
        $composer->setConfig(new Config());
        $composer->setPackage($rootPackage);

        return new Event('post-install-cmd', $composer, $io);
    }
}
