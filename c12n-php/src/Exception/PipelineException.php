<?php

declare(strict_types=1);

namespace HopTop\C12n\Exception;

/**
 * Thrown when the underlying FFI pipeline returns an error, fails to
 * allocate, or is invoked in an invalid lifecycle state (closed pipeline,
 * null handle, malformed JSON envelope, etc.).
 */
class PipelineException extends C12nException
{
}
