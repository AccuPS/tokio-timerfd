use crate::{ClockId, TimerFd};
use futures::{task, try_ready, Async, Future};
use std::io::Error as IoError;
use std::time::Instant;
use timerfd::{SetTimeFlags, TimerState};

/// A future that completes at a specified instant in time.
/// Instances of Delay perform no work and complete with () once the specified deadline has been reached.
/// Delay is powered by `timerfd` and has a resolution of 1 nanosecond.
pub struct Delay {
    timerfd: TimerFd,
    deadline: Instant,
    initialized: bool,
    task: Option<task::Task>,
}

impl Delay {
    /// Create a new `Delay` instance that elapses at `deadline`.
    pub fn new(deadline: Instant) -> Result<Self, IoError> {
        let timerfd = TimerFd::new(ClockId::Monotonic)?;
        Ok(Delay {
            timerfd,
            deadline,
            initialized: false,
            task: None,
        })
    }

    /// Returns the instant at which the future will complete.
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Returns true if the `Delay` has elapsed
    pub fn is_elapsed(&self) -> bool {
        self.deadline > Instant::now()
    }

    /// Reset the `Delay` instance to a new deadline.
    pub fn reset(&mut self, deadline: Instant) {
        self.deadline = deadline;
        self.initialized = false;
        if let Some(task) = &self.task {
            task.notify()
        }
    }
}

impl Future for Delay {
    type Item = ();
    type Error = IoError;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        if !self.initialized {
            let now = Instant::now();
            let duration = if self.deadline > now {
                self.deadline - now
            } else {
                return Ok(Async::Ready(()));
            };
            self.timerfd
                .set_state(TimerState::Oneshot(duration), SetTimeFlags::Default);
            self.initialized = true;
        }
        try_ready!(self.timerfd.poll_read());
        Ok(Async::Ready(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use tokio::prelude::*;

    #[test]
    fn delay_zero_duration() {
        tokio::run(future::lazy(|| {
            let now = Instant::now();
            let delay = Delay::new(Instant::now());
            delay
                .and_then(|_| {
                    let elapsed = now.elapsed();
                    println!("{:?}", elapsed);
                    assert!(elapsed < Duration::from_millis(1));
                    Ok(())
                })
                .map_err(|err| panic!("{:?}", err))
        }));
    }

    #[test]
    fn dropped_delay_doesnt_fire() {
        tokio::run(future::lazy(|| {
            let now = Instant::now();
            let mut delay_fired = false;
            let delay = Delay::new(now + Duration::from_millis(500))
                .unwrap()
                .and_then(move |_| {
                    delay_fired = true;
                    Ok(())
                });
            delay
                .select(future::ok(()))
                .and_then(move |_| {
                    assert_eq!(delay_fired, false);
                    Ok(())
                })
                .map_err(|_err| panic!())
        }))
    }

    #[test]
    fn delay_works() {
        tokio::run(future::lazy(|| {
            let now = Instant::now();
            let delay = Delay::new(now + Duration::from_micros(10));
            delay
                .and_then(|_| {
                    let elapsed = now.elapsed();
                    println!("{:?}", elapsed);
                    assert!(elapsed < Duration::from_millis(1));
                    Ok(())
                })
                .map_err(|err| panic!("{:?}", err))
        }));
    }
}
