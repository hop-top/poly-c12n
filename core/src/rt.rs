//! Target-portable runtime primitives: a monotonic clock and an async
//! timeout combinator.
//!
//! `pipeline.rs` needs two things from the platform: "what time is it"
//! (`Instant::now()` / `elapsed()`) and "give up on this future after
//! `Duration`". Native targets get both from tokio. `wasm32-unknown-unknown`
//! gets neither for free:
//!
//! * `std::time::Instant::now()` is stubbed to `unreachable!()`
//!   (`library/std/src/sys/time/unsupported.rs`).
//! * tokio's `time` feature builds a timer driver on top of that same
//!   stub, so merely calling `Builder::enable_all()` panics at runtime
//!   with `time not implemented on this platform` — before any signal
//!   ever runs.
//!
//! So on wasm32 the `time` feature is not enabled on tokio at all (see
//! `core/Cargo.toml`), `Instant` comes from the `instant` crate
//! (`performance.now()`-backed) and `timeout` is built on the host's
//! `setTimeout`, driven by the current-thread runtime's own waker.
//!
//! Semantics are identical across targets: `timeout(d, fut)` resolves to
//! `Ok(fut_output)` if the future finishes first, `Err(Elapsed)` otherwise.
//! Native behaviour is unchanged — it delegates straight to
//! `tokio::time::timeout`.

use std::future::Future;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;
#[cfg(target_arch = "wasm32")]
pub use instant::Instant;

/// Error returned by [`timeout`] when the inner future did not complete in
/// time. Mirrors `tokio::time::error::Elapsed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed;

impl std::fmt::Display for Elapsed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("deadline has elapsed")
    }
}

impl std::error::Error for Elapsed {}

/// Run `future`, giving up after `duration`.
#[cfg(not(target_arch = "wasm32"))]
pub async fn timeout<F: Future>(duration: Duration, future: F) -> Result<F::Output, Elapsed> {
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| Elapsed)
}

/// Run `future`, giving up after `duration`.
///
/// wasm32 flavour: races `future` against a `setTimeout`-backed sleep.
#[cfg(target_arch = "wasm32")]
pub async fn timeout<F: Future>(duration: Duration, future: F) -> Result<F::Output, Elapsed> {
    use std::pin::pin;
    use std::task::Poll;

    let mut future = pin!(future);
    let mut sleep = pin!(wasm_time::sleep(duration));

    std::future::poll_fn(move |cx| {
        if let Poll::Ready(out) = future.as_mut().poll(cx) {
            return Poll::Ready(Ok(out));
        }
        if sleep.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(Elapsed));
        }
        Poll::Pending
    })
    .await
}

/// `setTimeout`-backed sleep for `wasm32-unknown-unknown`.
///
/// Deliberately hand-rolled against the host's `setTimeout` rather than
/// pulling in `wasm-bindgen-futures` / `js-sys`: the only thing needed is
/// a one-shot timer that wakes the current-thread runtime, and going
/// through a JS `Promise` would additionally require a microtask executor.
#[cfg(target_arch = "wasm32")]
mod wasm_time {
    use std::cell::Cell;
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = setTimeout)]
        fn set_timeout(handler: &Closure<dyn FnMut()>, timeout_ms: f64) -> f64;

        #[wasm_bindgen(js_name = clearTimeout)]
        fn clear_timeout(handle: f64);
    }

    struct Shared {
        fired: Cell<bool>,
        waker: std::cell::RefCell<Option<Waker>>,
    }

    /// Future that resolves once the host timer fires.
    pub struct Sleep {
        shared: Rc<Shared>,
        handle: Option<f64>,
        // Kept alive for as long as the timer may fire; dropping the
        // closure while the timer is pending would leave JS calling into
        // freed wasm memory.
        _closure: Closure<dyn FnMut()>,
    }

    // SAFETY: `wasm32-unknown-unknown` is single-threaded — there is no
    // `std::thread`, and wasm-bindgen's generated glue is not reentrant
    // across agents. Nothing can observe an `Rc`/`Closure` from a second
    // thread, so the non-`Send` interior of `Sleep` cannot be raced.
    //
    // The assertion is needed (rather than making the whole `Signal` trait
    // `?Send` on wasm32) because `Signal` is `Send + Sync` on every target
    // and `Pipeline::evaluate` fans signals out through `JoinSet::spawn`,
    // which demands `Send` futures. Keeping one uniform trait shape across
    // native and wasm is what lets `pipeline.rs` stay target-agnostic.
    unsafe impl Send for Sleep {}
    unsafe impl Sync for Sleep {}

    /// Sleep for `duration` using the host's `setTimeout`.
    pub fn sleep(duration: Duration) -> Sleep {
        let shared = Rc::new(Shared {
            fired: Cell::new(false),
            waker: std::cell::RefCell::new(None),
        });

        let cb_shared = Rc::clone(&shared);
        let closure = Closure::<dyn FnMut()>::new(move || {
            cb_shared.fired.set(true);
            if let Some(waker) = cb_shared.waker.borrow_mut().take() {
                waker.wake();
            }
        });

        // `setTimeout` clamps to u32 ms; saturate rather than wrap so a
        // very long timeout stays "effectively never" instead of firing
        // immediately.
        let ms = duration.as_secs_f64() * 1000.0;
        let ms = ms.min(f64::from(u32::MAX));
        let handle = set_timeout(&closure, ms);

        Sleep {
            shared,
            handle: Some(handle),
            _closure: closure,
        }
    }

    impl Future for Sleep {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.shared.fired.get() {
                self.handle = None;
                return Poll::Ready(());
            }
            *self.shared.waker.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    impl Drop for Sleep {
        fn drop(&mut self) {
            // Cancel a still-pending timer so it can't fire into a dropped
            // closure (the inner future won the race).
            if let Some(handle) = self.handle.take() {
                clear_timeout(handle);
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timeout_returns_output_when_future_wins() {
        let out = timeout(Duration::from_secs(10), async { 42 }).await;
        assert_eq!(out, Ok(42));
    }

    #[tokio::test]
    async fn timeout_elapses_when_future_is_slow() {
        let out = timeout(
            Duration::from_millis(10),
            tokio::time::sleep(Duration::from_secs(30)),
        )
        .await;
        assert_eq!(out, Err(Elapsed));
    }

    #[test]
    fn instant_elapsed_is_monotonic() {
        let start = Instant::now();
        assert!(start.elapsed() >= Duration::ZERO);
    }
}
